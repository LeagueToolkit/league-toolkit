use crate::Span;

/// Like [`std::fmt::Debug`], with spans resolved against source `text`.
pub trait DebugSpans {
    fn fmt(&self, text: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;
}

pub trait SpannedToString {
    fn spanned_to_string(&self, text: &str) -> String;
}

impl<T: DebugSpans + ?Sized> SpannedToString for T {
    fn spanned_to_string(&self, text: &str) -> String {
        format!("{:?}", debug_with_source(self, text))
    }
}

pub fn debug_with_source<'a, T: DebugSpans + ?Sized>(
    value: &'a T,
    text: &'a str,
) -> impl std::fmt::Debug + 'a {
    __private::View { value, text }
}

impl DebugSpans for Span {
    fn fmt(&self, text: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "`{}`", &text[self])
    }
}

macro_rules! forward_deref {
    ($($ty:ty),* $(,)?) => {$(
        impl<T: DebugSpans + ?Sized> DebugSpans for $ty {
            fn fmt(&self, text: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                DebugSpans::fmt(&**self, text, f)
            }
        }
    )*};
}

forward_deref!(&T, &mut T, Box<T>, std::rc::Rc<T>, std::sync::Arc<T>);

impl<T: DebugSpans> DebugSpans for Option<T> {
    fn fmt(&self, text: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Some(value) => f
                .debug_tuple("Some")
                .field(&debug_with_source(value, text))
                .finish(),
            None => f.write_str("None"),
        }
    }
}

impl<T: DebugSpans> DebugSpans for [T] {
    fn fmt(&self, text: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.iter().map(|value| debug_with_source(value, text)))
            .finish()
    }
}

impl<T: DebugSpans> DebugSpans for Vec<T> {
    fn fmt(&self, text: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        DebugSpans::fmt(self.as_slice(), text, f)
    }
}

impl<T: DebugSpans, const N: usize> DebugSpans for [T; N] {
    fn fmt(&self, text: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        DebugSpans::fmt(self.as_slice(), text, f)
    }
}

macro_rules! forward_tuple {
    ($($name:ident.$idx:tt),+) => {
        impl<$($name: DebugSpans),+> DebugSpans for ($($name,)+) {
            fn fmt(&self, text: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_tuple("")
                    $(.field(&debug_with_source(&self.$idx, text)))+
                    .finish()
            }
        }
    };
}

forward_tuple!(A.0);
forward_tuple!(A.0, B.1);
forward_tuple!(A.0, B.1, C.2);
forward_tuple!(A.0, B.1, C.2, D.3);

#[doc(hidden)]
pub mod __private {
    use super::DebugSpans;
    use std::fmt::{self, Debug};

    pub struct View<'a, T: ?Sized> {
        pub value: &'a T,
        pub text: &'a str,
    }

    impl<T: DebugSpans + ?Sized> Debug for View<'_, T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            DebugSpans::fmt(self.value, self.text, f)
        }
    }

    pub struct PlainView<'a, T: ?Sized>(pub &'a T);

    impl<T: Debug + ?Sized> Debug for PlainView<'_, T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            Debug::fmt(self.0, f)
        }
    }

    pub struct Probe<'a, T: ?Sized>(pub &'a T, pub &'a str);

    pub trait SpannedField<'a> {
        type Out: Debug;
        fn dbg_field(&self) -> Self::Out;
    }

    impl<'a, T: DebugSpans + ?Sized> SpannedField<'a> for Probe<'a, T> {
        type Out = View<'a, T>;
        fn dbg_field(&self) -> View<'a, T> {
            View {
                value: self.0,
                text: self.1,
            }
        }
    }

    pub trait DebugField<'a> {
        type Out: Debug;
        fn dbg_field(&self) -> Self::Out;
    }

    impl<'a, T: Debug + ?Sized> DebugField<'a> for &Probe<'a, T> {
        type Out = PlainView<'a, T>;
        fn dbg_field(&self) -> PlainView<'a, T> {
            PlainView(self.0)
        }
    }

    pub fn spanned<'a, V: Debug + 'a>(
        span: super::Span,
        value: V,
        text: &'a str,
    ) -> impl Debug + 'a {
        struct Spanned<'a, V> {
            span: super::Span,
            value: V,
            text: &'a str,
        }

        impl<V: Debug> Debug for Spanned<'_, V> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct("Spanned")
                    .field(
                        "span",
                        &View {
                            value: &self.span,
                            text: self.text,
                        },
                    )
                    .field("value", &self.value)
                    .finish()
            }
        }

        Spanned { span, value, text }
    }
}
