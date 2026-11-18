//! The NovaSharp type system: interned types, canonical spellings, sizes and
//! name mangling primitives.
//!
//! Types are interned in a [`TypeStore`] so that type identity is a cheap
//! `TypeId` comparison. Structural types (arrays, pointers, functions) are
//! compared structurally; named types are nominal and refer to a declaration.

use std::fmt::Write as _;

/// Identity of an interned type.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TypeId(pub u32);

impl TypeId {
    /// The error type, used to keep analysis going after a type error.
    pub const ERROR: TypeId = TypeId(0);
    /// The empty type (`void`): the return type of functions without `: Type`.
    pub const VOID: TypeId = TypeId(1);
    /// `boolean`.
    pub const BOOLEAN: TypeId = TypeId(2);
}

/// One type.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Type {
    /// Poison type produced after a type error.
    Error,
    /// The empty type (§3: a function without a return type returns nothing).
    Void,
    /// `boolean`, one byte.
    Boolean,
    /// `integer(N)` (signed) or `cardinal(N)` (unsigned), N ∈ {1,2,4,8,16}.
    Integer {
        /// Whether the type is `integer` (`true`) or `cardinal` (`false`).
        signed: bool,
        /// Size in bytes.
        bytes: u8,
    },
    /// `float(N)`, N ∈ {4,8}.
    Float {
        /// Size in bytes.
        bytes: u8,
    },
    /// Fixed-size `array(dims) element`.
    Array {
        /// Dimension sizes, outermost first.
        dimensions: Vec<u64>,
        /// Element type.
        element: TypeId,
    },
    /// `pointer [target]`.
    Pointer {
        /// Pointed-to type; `None` when the target is omitted.
        target: Option<TypeId>,
    },
    /// `function(params) return`.
    Function {
        /// Parameter types (parameter names, defaults and `&` are not part of
        /// the type, §3.6.3).
        parameters: Vec<TypeId>,
        /// Whether the parameter list ends in `wildcard`.
        wildcard: bool,
        /// Explicit calling convention, if written.
        calling_convention: Option<i32>,
        /// Return type; `None` is the empty type.
        return_type: Option<TypeId>,
    },
    /// A variable-length array: `{ pointer, length }` (§3.6.1).
    Vla {
        /// Element type.
        element: TypeId,
        /// Type of the runtime length value.
        length: TypeId,
    },
    /// A declared type: index into the semantic analysis' type definitions.
    Named(u32),
}

impl Type {
    /// Returns whether this is the error type.
    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self, Type::Error)
    }

    /// Returns whether this is the empty type.
    #[must_use]
    pub const fn is_void(&self) -> bool {
        matches!(self, Type::Void)
    }
}

/// Interned types.
#[derive(Debug)]
pub struct TypeStore {
    types: Vec<Type>,
}

impl Default for TypeStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeStore {
    /// Creates a store containing the built-in error, empty and boolean types.
    #[must_use]
    pub fn new() -> Self {
        Self {
            types: vec![Type::Error, Type::Void, Type::Boolean],
        }
    }

    /// Interns `ty`, returning its id.
    pub fn intern(&mut self, ty: Type) -> TypeId {
        if let Some(index) = self.types.iter().position(|existing| *existing == ty) {
            return TypeId(index as u32);
        }
        let id = TypeId(self.types.len() as u32);
        self.types.push(ty);
        id
    }

    /// Returns the type with `id`.
    #[must_use]
    pub fn get(&self, id: TypeId) -> &Type {
        &self.types[id.0 as usize]
    }

    /// Interns an `integer` or `cardinal` type.
    pub fn integer(&mut self, signed: bool, bytes: u8) -> TypeId {
        self.intern(Type::Integer { signed, bytes })
    }

    /// Interns a `float` type.
    pub fn float(&mut self, bytes: u8) -> TypeId {
        self.intern(Type::Float { bytes })
    }

    /// Interns a fixed-size array type.
    pub fn array(&mut self, dimensions: Vec<u64>, element: TypeId) -> TypeId {
        // Nested fixed arrays are one array with combined dimensions
        // (`array(2) array(2) T` == `array(2,2) T`, §3.6.1), so intern both
        // spellings as the same type.
        let mut dimensions = dimensions;
        let mut element = element;
        loop {
            match self.get(element).clone() {
                Type::Array {
                    dimensions: inner,
                    element: inner_element,
                } => {
                    dimensions.extend(inner);
                    element = inner_element;
                }
                _ => break,
            }
        }
        self.intern(Type::Array {
            dimensions,
            element,
        })
    }

    /// Interns a pointer type.
    pub fn pointer(&mut self, target: Option<TypeId>) -> TypeId {
        self.intern(Type::Pointer { target })
    }

    /// Interns a function type.
    pub fn function(
        &mut self,
        parameters: Vec<TypeId>,
        wildcard: bool,
        calling_convention: Option<i32>,
        return_type: Option<TypeId>,
    ) -> TypeId {
        self.intern(Type::Function {
            parameters,
            wildcard,
            calling_convention,
            return_type,
        })
    }

    /// Canonical spelling of a type, as used by name mangling (§13.2: no
    /// whitespace is allowed in symbol names).
    #[must_use]
    pub fn spelling(&self, id: TypeId) -> String {
        let mut out = String::new();
        self.write_spelling(id, &mut out);
        out
    }

    fn write_spelling(&self, id: TypeId, out: &mut String) {
        match self.get(id) {
            Type::Error | Type::Void => out.push_str("void"),
            Type::Boolean => out.push_str("boolean"),
            Type::Integer { signed, bytes } => {
                let name = if *signed { "integer" } else { "cardinal" };
                let _ = write!(out, "{name}({bytes})");
            }
            Type::Float { bytes } => {
                let _ = write!(out, "float({bytes})");
            }
            Type::Array {
                dimensions,
                element,
            } => {
                out.push_str("array(");
                for (index, dimension) in dimensions.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    let _ = write!(out, "{dimension}");
                }
                out.push(')');
                self.write_spelling(*element, out);
            }
            Type::Vla { element, .. } => {
                out.push_str("array(runtime)");
                self.write_spelling(*element, out);
            }
            Type::Pointer { target } => {
                out.push_str("pointer");
                if let Some(target) = target {
                    self.write_spelling(*target, out);
                }
            }
            Type::Function {
                parameters,
                wildcard,
                calling_convention,
                return_type,
            } => {
                out.push_str("function(");
                for (index, parameter) in parameters.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    self.write_spelling(*parameter, out);
                }
                if *wildcard {
                    out.push_str(",wildcard");
                }
                if let Some(convention) = calling_convention {
                    let _ = write!(out, ";{convention}");
                }
                out.push(')');
                if let Some(return_type) = return_type {
                    self.write_spelling(*return_type, out);
                }
            }
            Type::Named(_) => {
                // The declaration's canonical name is filled in by the semantic
                // analysis through `qualified_name`; without it the index is
                // the best canonical spelling available.
                let _ = write!(out, "named({})", id.0);
            }
        }
    }
}

/// Escapes one symbol-text component (§13.2): `A-Za-z0-9` stay, `_` becomes
/// `_5f`, every other byte becomes `_` plus two lowercase hex digits.
#[must_use]
pub fn escape_symbol_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() {
            out.push(char::from(byte));
        } else if byte == b'_' {
            out.push_str("_5f");
        } else {
            let _ = write!(out, "_{byte:02x}");
        }
    }
    out
}

/// Encodes one symbol component as `length + escaped text` (§13.2).
#[must_use]
pub fn encode_symbol_component(text: &str) -> String {
    let escaped = escape_symbol_text(text);
    let mut out = String::with_capacity(escaped.len() + 4);
    let _ = write!(out, "{}", escaped.len());
    out.push_str(&escaped);
    out
}

/// FNV-1a 64-bit hash with the constants fixed by §13.2.
#[must_use]
pub fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Mangles a function symbol name (§13.2).
///
/// `scope` is the module path of the declaration, `name` the declared name and
/// `parameters` the canonical spellings of the parameter types (`this` first for
/// methods).
#[must_use]
pub fn mangle_function(scope: &[String], name: &str, parameters: &[String]) -> String {
    let mut symbol = String::from("N");
    symbol.push_str(&encode_symbol_component(&scope.join(".")));
    symbol.push_str(&encode_symbol_component(name));

    let mut hash_input = String::new();
    for parameter in parameters {
        hash_input.push_str(&encode_symbol_component(parameter));
    }
    let _ = write!(symbol, "{:016x}", fnv1a_64(hash_input.as_bytes()));
    symbol
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_spec_mangling_example() {
        // §13.2: scope `Main`, name `Compute`, parameters `integer(4)` must give
        // `N4Main7Compute2bf5d948506c5341`.
        let symbol = mangle_function(
            &["Main".to_string()],
            "Compute",
            &["integer(4)".to_string()],
        );
        assert_eq!(symbol, "N4Main7Compute2bf5d948506c5341");
    }

    #[test]
    fn escapes_symbol_text() {
        assert_eq!(escape_symbol_text("integer(4)"), "integer_284_29");
        assert_eq!(escape_symbol_text("a_b"), "a_5fb");
        assert_eq!(escape_symbol_text("A.B"), "A_2eB");
    }

    #[test]
    fn encodes_components() {
        assert_eq!(encode_symbol_component("integer(4)"), "14integer_284_29");
        assert_eq!(encode_symbol_component(""), "0");
    }

    #[test]
    fn canonical_spellings_have_no_whitespace() {
        let mut types = TypeStore::new();
        let integer = types.integer(true, 4);
        let array = types.array(vec![3], integer);
        assert_eq!(types.spelling(integer), "integer(4)");
        assert_eq!(types.spelling(array), "array(3)integer(4)");
        let pointer = types.pointer(Some(integer));
        assert_eq!(types.spelling(pointer), "pointerinteger(4)");
        let function = types.function(vec![integer], false, None, Some(integer));
        assert_eq!(types.spelling(function), "function(integer(4))integer(4)");
    }
}
