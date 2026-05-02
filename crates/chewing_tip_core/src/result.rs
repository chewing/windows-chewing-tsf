use std::{error::Error, fmt::Display, marker::PhantomData};

pub trait ErrorWithContextBuilder {
    type Error;
    fn with_msg(self, msg: &'static str) -> Self;
    fn with_src(self, src: Box<dyn Error + Send + Sync + 'static>) -> Self;
    fn build(self) -> Self::Error;
}

pub trait ErrorWithContext {
    type Builder: ErrorWithContextBuilder<Error = Self>;
}

pub struct FromBuilder<T> {
    msg: Option<&'static str>,
    src: Option<Box<dyn Error + Send + Sync + 'static>>,
    err: PhantomData<T>,
}

impl<T> Default for FromBuilder<T> {
    fn default() -> Self {
        FromBuilder {
            msg: None,
            src: None,
            err: PhantomData,
        }
    }
}

impl<T> ErrorWithContextBuilder for FromBuilder<T>
where
    T: From<(&'static str, Box<dyn Error + Send + Sync + 'static>)>,
{
    type Error = T;
    fn with_msg(mut self, msg: &'static str) -> Self {
        self.msg = Some(msg);
        self
    }
    fn with_src(mut self, src: Box<dyn Error + Send + Sync + 'static>) -> Self {
        self.src = Some(src);
        self
    }
    fn build(self) -> Self::Error {
        T::from((self.msg.unwrap(), self.src.unwrap()))
    }
}

#[macro_export]
macro_rules! impl_error_from {
    ($msg:ident, $source:ident, $error_type:ident, $ctor:expr) => {
        impl
            From<(
                &'static str,
                Box<dyn std::error::Error + Send + Sync + 'static>,
            )> for $error_type
        {
            fn from(
                value: (
                    &'static str,
                    Box<dyn std::error::Error + Send + Sync + 'static>,
                ),
            ) -> Self {
                let $msg = value.0;
                let $source = value.1;
                $ctor
            }
        }
        impl $crate::result::ErrorWithContext for $error_type {
            type Builder = $crate::result::FromBuilder<$error_type>;
        }
    };
}

#[macro_export]
macro_rules! impl_context_error {
    ($error_type:ident) => {
        impl_context_error!(pub(crate) $error_type);
    };
    ($vis:vis $error_type:ident) => {
        #[derive(Debug)]
        $vis struct $error_type {
            msg: &'static str,
            source: Box<dyn std::error::Error + Send + Sync + 'static>,
        }
        impl std::error::Error for $error_type {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(self.source.as_ref())
            }
        }
        impl std::fmt::Display for $error_type {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.msg)
            }
        }
        $crate::impl_error_from!(msg, source, $error_type, $error_type { msg, source });
    };
}

#[inline]
pub fn expect_error<T, E>(
    msg: &'static str,
    body: impl FnOnce() -> Result<T, Box<dyn Error + Send + Sync + 'static>>,
) -> Result<T, E>
where
    E: From<(&'static str, Box<dyn Error + Send + Sync + 'static>)>,
    E: ErrorWithContext<Builder = FromBuilder<E>>,
{
    body().map_err(|e| {
        FromBuilder::<E>::default()
            .with_msg(msg)
            .with_src(e)
            .build()
    })
}

#[inline]
pub fn expect_error_builder<T, E, EB>(
    msg: &'static str,
    builder: EB,
    body: impl FnOnce() -> Result<T, Box<dyn Error + Send + Sync + 'static>>,
) -> Result<T, E>
where
    E: ErrorWithContext<Builder = EB>,
    EB: ErrorWithContextBuilder<Error = E>,
{
    body().map_err(|e| builder.with_msg(msg).with_src(e).build())
}

pub struct Report<'a, T>(pub &'a T);

impl<T> Display for Report<'_, T>
where
    T: Error,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Error: {}", self.0)?;
        if self.0.source().is_some() {
            write!(f, "Caused by:")?;
            let mut index = 0;
            let mut parent = self.0.source();
            while let Some(src) = parent {
                write!(f, "\n    {index}: {}", src)?;
                index += 1;
                parent = src.source();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Display;

    use super::Report;

    #[derive(Debug)]
    struct Error(&'static str, Box<dyn std::error::Error>);
    impl std::error::Error for Error {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(self.1.as_ref())
        }
    }
    impl Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(self.0)
        }
    }

    #[test]
    fn test_report() {
        let errors = Error(
            "Failed to do something",
            Error(
                "Failed due to internal error",
                std::io::Error::other("Failed to perform IO").into(),
            )
            .into(),
        );
        assert_eq!(
            Report(&errors).to_string(),
            "Error: Failed to do something\nCaused by:\n    0: Failed due to internal error\n    1: Failed to perform IO"
        );
    }
}
