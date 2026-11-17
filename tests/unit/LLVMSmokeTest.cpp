// Smoke test for the LLVM back end plumbing: build a module, verify it, print
// textual IR, round-trip it through the IR reader, and emit a host object file
// through the target machine. If this test passes, the compiler's eventual
// code generation layer has everything it needs at link time.

#include "Testing.h"

#include "llvm/ADT/SmallString.h"
#include "llvm/AsmParser/Parser.h"
#include "llvm/IR/IRBuilder.h"
#include "llvm/IR/LLVMContext.h"
#include "llvm/IR/LegacyPassManager.h"
#include "llvm/IR/Module.h"
#include "llvm/IR/Verifier.h"
#include "llvm/IRReader/IRReader.h"
#include "llvm/MC/TargetRegistry.h"
#include "llvm/Support/CodeGen.h"
#include "llvm/Support/SourceMgr.h"
#include "llvm/Support/TargetSelect.h"
#include "llvm/Support/raw_ostream.h"
#include "llvm/Target/TargetMachine.h"
#include "llvm/Target/TargetOptions.h"
#include "llvm/TargetParser/Host.h"

#include <memory>
#include <string>

using namespace ainsc;

namespace {

/// int answer() { return 42; }
std::unique_ptr<llvm::Module> buildAnswerModule(llvm::LLVMContext &context) {
  auto module = std::make_unique<llvm::Module>("ainsc_smoke", context);
  llvm::IRBuilder<> builder(context);

  llvm::FunctionType *signature = llvm::FunctionType::get(builder.getInt32Ty(), false);
  llvm::Function *function =
      llvm::Function::Create(signature, llvm::Function::ExternalLinkage, "answer", *module);
  llvm::BasicBlock *entry = llvm::BasicBlock::Create(context, "entry", function);

  builder.SetInsertPoint(entry);
  builder.CreateRet(builder.getInt32(42));
  return module;
}

} // namespace

AINSC_TEST(LLVM, VerifyPrintAndRoundTripIR) {
  llvm::LLVMContext context;
  const std::unique_ptr<llvm::Module> module = buildAnswerModule(context);

  std::string verifyMessage;
  llvm::raw_string_ostream verifyStream(verifyMessage);
  const bool broken = llvm::verifyModule(*module, &verifyStream);
  verifyStream.flush();
  AINSC_CHECK(!broken);
  AINSC_CHECK(verifyMessage.empty());

  std::string text;
  llvm::raw_string_ostream textStream(text);
  module->print(textStream, /*AssemblyAnnotationWriter=*/nullptr);
  textStream.flush();
  AINSC_CHECK(text.find("define i32 @answer()") != std::string::npos);

  // Feed the textual IR back into the IR reader.
  llvm::SMDiagnostic diagnostic;
  llvm::LLVMContext roundTripContext;
  const std::unique_ptr<llvm::Module> parsed =
      llvm::parseAssemblyString(text, diagnostic, roundTripContext);
  AINSC_CHECK(parsed != nullptr);
  if (parsed) {
    AINSC_CHECK_EQ(parsed->getFunction("answer") == nullptr, false);
  }
}

AINSC_TEST(LLVM, EmitObjectFileForHost) {
  llvm::InitializeNativeTarget();
  llvm::InitializeNativeTargetAsmPrinter();
  llvm::InitializeNativeTargetAsmParser();

  llvm::LLVMContext context;
  const std::unique_ptr<llvm::Module> module = buildAnswerModule(context);

  const std::string triple = llvm::sys::getDefaultTargetTriple();
  std::string lookupError;
  const llvm::Target *target = llvm::TargetRegistry::lookupTarget(triple, lookupError);
  AINSC_CHECK(target != nullptr);
  if (target == nullptr) {
    return;
  }

  std::unique_ptr<llvm::TargetMachine> targetMachine(target->createTargetMachine(
      triple, /*CPU=*/"generic", /*Features=*/"", llvm::TargetOptions(), std::nullopt));
  AINSC_CHECK(targetMachine != nullptr);
  if (targetMachine == nullptr) {
    return;
  }

  module->setTargetTriple(triple);
  module->setDataLayout(targetMachine->createDataLayout());

  llvm::SmallString<4096> object;
  llvm::raw_svector_ostream objectStream(object);
  llvm::legacy::PassManager passManager;
  const llvm::CodeGenFileType fileType = llvm::CodeGenFileType::ObjectFile;
  const bool failedToAddPasses =
      targetMachine->addPassesToEmitFile(passManager, objectStream, /*DwoOut=*/nullptr, fileType);
  AINSC_CHECK(!failedToAddPasses);
  if (failedToAddPasses) {
    return;
  }
  passManager.run(*module);

  AINSC_CHECK(object.size() > 4);
  // Host is x86-64 Linux: the object must start with the ELF magic number.
  if (object.size() >= 4) {
    AINSC_CHECK_EQ(static_cast<unsigned char>(object[0]), 0x7Fu);
    AINSC_CHECK_EQ(static_cast<unsigned char>(object[1]), static_cast<unsigned char>('E'));
    AINSC_CHECK_EQ(static_cast<unsigned char>(object[2]), static_cast<unsigned char>('L'));
    AINSC_CHECK_EQ(static_cast<unsigned char>(object[3]), static_cast<unsigned char>('F'));
  }
}
