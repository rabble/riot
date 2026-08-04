#![doc(html_logo_url = "https://willowprotocol.org/assets/willow_emblem_standalone.png")]
#![cfg_attr(not(feature = "std"), no_std)]

//! An implementation of the [Willow specifications](https://willowprotocol.org/).
//!
//! See the [tutorials](https://willowprotocol.org/rust/) if you prefer hands-on learning over API docs.
//!
//! ## Contents
//!
//! This crate is grouped into several modules, each of which contains further information on how things are structured.
//!
//! The [`prelude`] module provides a convenient bulk import for all things Willow: `use willow25::prelude::*;`.
//!
//! The [`paths`] module provides functionality around [Paths](https://willowprotocol.org/specs/data-model/index.html#Path).
//!
//! The [`entry`] module provides functionality around [Entries](https://willowprotocol.org/specs/data-model/index.html#Entry).
//!
//! The [`groupings`] module provides functionality around [groupings](https://willowprotocol.org/specs/grouping-entries/).
//!
//! The [`authorisation`] module provides functionality around authentication of entries and read access.
//!
//! The [`storage`] module provides funcitonality for working with [stores](https://willowprotocol.org/specs/data-model/index.html#store), i.e., for storing entries.
//!
//! ## Willow’25 vs Willow
//!
//! The Willow specifications are generic over certain choices of parameters. [Willow25](https://willowprotocol.org/specs/willow25) is a specific choice of these parameters. As such, this crate is ready to use as-is, there are no parameters to configure. If you do need the full genericity of Willow, see the [`willow_data_model`] crate.

extern crate alloc;

// Macro for internal use, quickly and consistently create willw25-specific wrapper types around the generic will implementations which actually power this crate.
macro_rules! wrapper {
    ($(#[$attr:meta])* $new:ident; $wrapped:ty ) => {
        wrapper! {$(#[$attr])* $new; $wrapped; dst}

        impl core::convert::From<$wrapped> for $new {
            fn from(value: $wrapped) -> Self {
                Self(value)
            }
        }

        impl core::convert::From<$new> for $wrapped {
            fn from(value: $new) -> Self {
                value.0
            }
        }
    };

    ($(#[$attr:meta])* $new:ident; $wrapped:ty; dst ) => {
        $(#[$attr])*
        #[repr(transparent)]
        pub struct $new(pub(crate) $wrapped);

        impl<'a> core::convert::From<&'a $wrapped> for &'a $new {
            fn from(value: &'a $wrapped) -> Self {
                // Safe because `$new` is `#[rep(transparent)]`.
                unsafe { &*(value as *const $wrapped as *const $new) }
            }
        }

        impl<'a> core::convert::From<&'a mut $wrapped> for &'a mut $new {
            fn from(value: &'a mut $wrapped) -> Self {
                // Safe because `$new` is `#[rep(transparent)]`.
                unsafe { &mut *(value as *mut $wrapped as *mut $new) }
            }
        }

        impl<'a> core::convert::From<&'a $new> for &'a $wrapped {
            fn from(value: &'a $new) -> Self {
                &value.0
            }
        }

        impl<'a> core::convert::From<&'a mut $new> for &'a mut $wrapped {
            fn from(value: &'a mut $new) -> Self {
                &mut value.0
            }
        }
    };

    ($(#[$attr:meta])* $new:ident <$($T:ident: $T_bound:path = $T_default:ty),*>; $wrapped:ty ) => {
        $(#[$attr])*
        #[repr(transparent)]
        pub struct $new<$($T: $T_bound = $T_default,)*>(pub(crate) $wrapped);

        impl<$($T: $T_bound,)*> core::convert::From<$wrapped> for $new<$($T,)*> {
            fn from(value: $wrapped) -> Self {
                Self(value)
            }
        }

        impl<$($T: $T_bound,)*> core::convert::From<$new<$($T,)*>> for $wrapped {
            fn from(value: $new<$($T,)*>) -> Self {
                value.0
            }
        }
    };
}

/// Creates a statically-known [`Component`](prelude::Component).
///
/// Use this macro when you need to create a statically known path component. You can specify the component as ascii:
///
/// ```
/// # use willow25::prelude::*;
/// assert_eq!(component!("abc123-._~").as_bytes(), b"abc123-._~");
/// ```
///
/// Bytes which are neither ascii alphanumerics nor the ascii encoding of one of `-._~` must be [percent-encoded](https://datatracker.ietf.org/doc/html/rfc3986#section-2.1): to encode the byte with hex representation `xy` (where `x` and `y` are ascii hex digits), write `%xy`. For example, to encode a forward slash (`/`), use `%2f`:
///
/// ```
/// # use willow25::prelude::*;
/// assert_eq!(component!("abc%2fdef").as_bytes(), b"abc/def");
/// ```
///
/// The macro causes a compile-time error if the supplied component is invalid.
///
/// ```compile_fail
/// # use willow25::prelude::*;
/// // Component contains a character which must be percent-encoded.
/// let nope = component!(":");
/// ```
///
/// ```compile_fail
/// # use willow25::prelude::*;
/// // Component contains an invalid percent-encoding.
/// let nope = component!("%az");
/// ```
///
/// ```compile_fail
/// # use willow25::prelude::*;
/// // Component contains a character which must be percent-encoded.
/// let nope = component!(":");
/// ```
///
/// ```compile_fail
/// # use willow25::prelude::*;
/// // Component length must not exceed 4096 bytes.
/// let nope = component!("too_loooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooong");
/// ```
#[macro_export]
macro_rules! component {
    ( $l:literal ) => {
        $crate::prelude::Component::new($crate::component_internal!($l)).unwrap()
    };
}

/// Creates a statically-known [`Path`](prelude::Path).
///
/// Use this macro when you need to create a statically known path. A `/` indicates a new component, the component contents can be specified as ascii:
///
/// ```
/// # use willow25::prelude::*;
/// assert_eq!(path!("/blog/ideas"), Path::from_slices(&[b"blog", b"ideas"])?);
/// # Ok::<(), PathError>(())
/// ```
///
/// Components have the same syntax as the [`component!`](component) macro: their bytes can be given as ascii if they are ascii alphanumerics or the ascii encoding of one of `-._~`, all other bytes must be [percent-encoded](https://datatracker.ietf.org/doc/html/rfc3986#section-2.1).
///
/// ```
/// # use willow25::prelude::*;
/// assert_eq!(path!("/abc%3d/def%2f"), Path::from_slices(&[b"abc=", b"def/"])?);
/// # Ok::<(), PathError>(())
/// ```
///
/// The empty path is given via the empty string literal, a single slash results in the path consisting of exactly one empty component, and so on.
///
/// ```
/// # use willow25::prelude::*;
/// assert_eq!(path!(""), Path::from_slices(&[])?);
/// assert_eq!(path!("/"), Path::from_slices(&[&[]])?);
/// assert_eq!(path!("//"), Path::from_slices(&[&[], &[]])?);
/// assert_eq!(path!("/hi/"), Path::from_slices(&[b"hi".as_slice(), &[].as_slice()])?);
/// # Ok::<(), PathError>(())
/// ```
///
/// The macro causes a compile-time error if the supplied path is invalid.
///
/// ```compile_fail
/// # use willow25::prelude::*;
/// // Component length must not exceed 4096 bytes.
/// let nope = path!("/too_loooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooong");
/// ```
///
/// ```compile_fail
/// # use willow25::prelude::*;
/// // Component count must not exceed 4096.
/// let nope = path!("/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////");
/// ```
///
/// ```compile_fail
/// # use willow25::prelude::*;
/// // Total path length must not exceed 4096 bytes.
/// let nope = path!("/tooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooo/loooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooooong");
/// ```
#[macro_export]
macro_rules! path {
    ( $l:literal ) => {
        $crate::prelude::Path::from_slices($crate::path_internal!($l)).unwrap()
    };
}

/// This is an implementation detail of a macro. Scoping things is a bit awkward in the world of macros sometimes. Please just pretend you never saw this, okay?
#[doc(hidden)]
pub use willow25_macros::component_internal;

/// This is an implementation detail of a macro. Scoping things is a bit awkward in the world of macros sometimes. Please just pretend you never saw this, okay?
#[doc(hidden)]
pub use willow25_macros::path_internal;

pub mod authorisation;
pub mod defaults;

#[cfg(feature = "drop_format")]
pub mod drop_format;

pub mod entry;
pub mod groupings;
pub mod paths;
pub mod storage;

/// A “prelude” for crates using the `willow25` crate.
///
/// This prelude is similar to the standard library’s prelude in that you’ll almost always want to import its entire contents, but unlike the standard library’s prelude you’ll have to do so manually:
///
/// `use willow25::prelude::*;`
///
/// The prelude may grow over time.
pub mod prelude {
    // pub use super::entry::{Entry, EntryBuilder, Entrylike, EntrylikeExt, Keylike, Timestamp};
    // pub use super::ordering::{Maximum, Minimum, Successor};
    pub use super::authorisation::{
        AuthorisationToken, AuthorisedEntry, ReadCapability, WriteCapability,
    };
    pub use super::entry::*;
    pub use super::groupings::*;
    pub use super::paths::{Component, OwnedComponent, Path, PathBuilder, component, path};
    pub use super::storage::{PayloadPrefixStore, Store};
    pub use super::{MCC, MCL, MPL};
    pub use hifitime::prelude::*;
    pub use meadowcap::DoesNotAuthorise;
    pub use willow_data_model::prelude::{
        InvalidComponentError, PathError, PathFromComponentsError,
    };
}

/// The [max\_component\_length](https://willowprotocol.org/specs/data-model/index.html#max_component_length) of [Willow’25](https://willowprotocol.org/specs/willow25/index.html#willow25_data_model): `4096`.
pub const MCL: usize = 4096;

/// The [max\_component\_count](https://willowprotocol.org/specs/data-model/index.html#max_component_count) of [Willow’25](https://willowprotocol.org/specs/willow25/index.html#willow25_data_model): `4096`.
pub const MCC: usize = 4096;

/// The [max\_path\_length](https://willowprotocol.org/specs/data-model/index.html#max_path_length) of [Willow’25](https://willowprotocol.org/specs/willow25/index.html#willow25_data_model): `4096`.
pub const MPL: usize = 4096;

/// Checks if a bit of a given byte is flagged at a particular index (with 0 being the most significant bit).
pub(crate) fn is_bitflagged(byte: u8, index: usize) -> bool {
    byte & (1u8 << (7 - index)) != 0u8
}
