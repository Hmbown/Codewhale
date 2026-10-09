use std::ops::{BitAnd, BitOr, BitXor};

use crate::style::Attribute;

/// a bitset for all possible attributes
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Attributes(u32);

impl From<Attribute> for Attributes {
    fn from(attribute: Attribute) -> Self {
        Self(attribute.bytes())
    }
}

impl From<&[Attribute]> for Attributes {
    fn from(arr: &[Attribute]) -> Self {
        let mut attributes = Attributes::default();
        for &attr in arr {
            attributes.set(attr);
        }
        attributes
    }
}

impl BitAnd<Attribute> for Attributes {
    type Output = Self;
    fn bitand(self, rhs: Attribute) -> Self {
        Self(self.0 & rhs.bytes())
    }
}
impl BitAnd for Attributes {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl BitOr<Attribute> for Attributes {
    type Output = Self;
    fn bitor(self, rhs: Attribute) -> Self {
        Self(self.0 | rhs.bytes())
    }
}
impl BitOr for Attributes {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitXor<Attribute> for Attributes {
    type Output = Self;
    fn bitxor(self, rhs: Attribute) -> Self {
        Self(self.0 ^ rhs.bytes())
    }
}
impl BitXor for Attributes {
    type Output = Self;
    fn bitxor(self, rhs: Self) -> Self {
        Self(self.0 ^ rhs.0)
    }
}

impl Attributes {
    /// Returns the empty bitset.
    #[inline(always)]
    pub const fn none() -> Self {
        Self(0)
    }

    /// Returns a copy of the bitset with the given attribute set.
    /// If it's already set, this returns the bitset unmodified.
    #[inline(always)]
    pub const fn with(self, attribute: Attribute) -> Self {
        Self(self.0 | attribute.bytes())
    }

    /// Returns a copy of the bitset with the given attribute unset.
    /// If it's not set, this returns the bitset unmodified.
    #[inline(always)]
    pub const fn without(self, attribute: Attribute) -> Self {
        Self(self.0 & !attribute.bytes())
    }

    /// Sets the attribute.
    /// If it's already set, this does nothing.
    #[inline(always)]
    pub fn set(&mut self, attribute: Attribute) {
        self.0 |= attribute.bytes();
    }

    /// Unsets the attribute.
    /// If it's not set, this changes nothing.
    #[inline(always)]
    pub fn unset(&mut self, attribute: Attribute) {
        self.0 &= !attribute.bytes();
    }

    /// Sets the attribute if it's unset, unset it
    /// if it is set.
    #[inline(always)]
    pub fn toggle(&mut self, attribute: Attribute) {
        self.0 ^= attribute.bytes();
    }

    /// Returns whether the attribute is set.
    #[inline(always)]
    pub const fn has(self, attribute: Attribute) -> bool {
        self.0 & attribute.bytes() != 0
    }

    /// Sets all the passed attributes. Removes none.
    #[inline(always)]
    pub fn extend(&mut self, attributes: Attributes) {
        self.0 |= attributes.0;
    }

    /// Returns whether there is no attribute set.
    #[inline(always)]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[cfg(test)]
mod tests {
    use super::{Attribute, Attributes};

    #[test]
    fn test_attributes() {
        let mut attributes: Attributes = Attribute::Bold.into();
        assert!(attributes.has(Attribute::Bold));
        attributes.set(Attribute::Italic);
        assert!(attributes.has(Attribute::Italic));
        attributes.unset(Attribute::Italic);
        assert!(!attributes.has(Attribute::Italic));
        attributes.toggle(Attribute::Bold);
        assert!(attributes.is_empty());
    }

    #[test]
    fn test_attributes_const() {
        const ATTRIBUTES: Attributes = Attributes::none()
            .with(Attribute::Bold)
            .with(Attribute::Italic)
            .without(Attribute::Bold);
        assert!(!ATTRIBUTES.has(Attribute::Bold));
        assert!(ATTRIBUTES.has(Attribute::Italic));
    }
}
