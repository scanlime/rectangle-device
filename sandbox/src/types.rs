use regex::Regex;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SandboxError {
    #[error("the string `{0}` is not a valid image digest")]
    InvalidDigest(String),
    #[error("the string `{0}` is not a valid image name")]
    InvalidImage(String),
    #[error("unexpected digest `{0}`")]
    UnexpectedDigest(String),
}

#[derive(Clone, Debug)]
pub struct ImageDigest {
    pub image: Image,
    pub digest: Digest,
}

impl ImageDigest {
    pub fn parse(image: &str, digest: &str) -> Result<ImageDigest, SandboxError> {
        Ok(ImageDigest {
            image: Image::parse(image)?,
            digest: Digest::parse(digest)?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Image {
    inner: String,
}

impl Image {
    pub fn parse(s: &str) -> Result<Image, SandboxError> {
        lazy_static! {
            static ref RE: Regex = Regex::new(
                // https://github.com/docker/distribution/blob/master/reference/regexp.go
                r"(?x)^
                (?:                     # Optional domain
                    (?:                 # First domain component
                        [a-zA-Z0-9] |
                        [a-zA-Z0-9][a-zA-Z0-9-]*[a-zA-Z0-9]
                    )
                    (?:                 # Optional additional domain components
                        \.
                        (?:
                            [a-zA-Z0-9] |
                            [a-zA-Z0-9][a-zA-Z0-9-]*[a-zA-Z0-9]
                        )
                    )*
                    (?:                 # Optional port number
                        :[0-9]+
                    )?
                    /
                )?
                (?:                     # Main name component
                    [a-z0-9]+
                    (?:
                        (?: [._] | __ | [-]* )
                        [a-z0-9]+
                    )*
                )
                (?:                     # Optional additional name components
                    /
                    [a-z0-9]+
                    (?:
                        (?: [._] | __ | [-]* )
                        [a-z0-9]+
                    )*
                )*
                (?:                     # Optional tag
                    :
                    [\w][\w.-]*
                )?
            $").unwrap();
        }
        let inner = s.to_owned();
        if RE.is_match(&inner) {
            Ok(Image { inner })
        } else {
            Err(SandboxError::InvalidImage(inner))
        }
    }

    pub fn as_str(&self) -> &str {
        self.inner.as_str()
    }
}

#[derive(Clone, Debug)]
pub struct Digest {
    inner: String,
}

impl Digest {
    pub fn parse(s: &str) -> Result<Digest, SandboxError> {
        lazy_static! {
            static ref RE: Regex = Regex::new(
                // https://github.com/docker/distribution/blob/master/reference/regexp.go
                r"(?x)^
                    (?:                 # Optional type
                        [A-Za-z][A-Za-z0-9]*
                        (?:
                            [-_+.][A-Za-z][A-Za-z0-9]*
                        )*
                        :
                    )?
                    [[:xdigit:]]{32,}   # At least a 32-digit hex number
                $").unwrap();
        }
        let inner = s.to_owned();
        if RE.is_match(&inner) {
            Ok(Digest { inner })
        } else {
            Err(SandboxError::InvalidDigest(inner))
        }
    }

    pub fn as_str(&self) -> &str {
        self.inner.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_image_name() {
        assert!(Image::parse("balls").is_ok());
        assert!(Image::parse("balls/").is_err());
        assert!(Image::parse("balls/etc").is_ok());
        assert!(Image::parse("balls/etc/and/more").is_ok());
        assert!(Image::parse("b-a-l-l-s").is_ok());
        assert!(Image::parse("-balls").is_err());
        assert!(Image::parse("--balls").is_err());
        assert!(Image::parse("b--alls").is_ok());
        assert!(Image::parse("balls.io/image/of/my/balls").is_ok());
        assert!(Image::parse("balls.io/image/of/my/balls:").is_err());
        assert!(Image::parse("balls.io/image/of/my/balls:?").is_err());
        assert!(Image::parse("balls.io/image/of/my/balls:0").is_ok());
        assert!(Image::parse("balls.io/image/of/my/balls:.").is_err());
        assert!(Image::parse("balls.io/image/of/my/balls:0.0").is_ok());
        assert!(Image::parse("balls.io/image/of//balls").is_err());
        assert!(Image::parse(" balls").is_err());
        assert!(Image::parse("balls ").is_err());
        assert!(Image::parse("balls:69").is_ok());
        assert!(Image::parse("balls:6.9").is_ok());
        assert!(Image::parse("balls:").is_err());
        assert!(Image::parse("balls.io:69/ball").is_ok());
        assert!(Image::parse("balls.io:/ball").is_err());
    }

    #[test]
    fn parse_digest_name() {
        assert!(Digest::parse("balls").is_err());
        assert!(Digest::parse("balls:0123456789abcdef0123456789abcdef").is_ok());
        assert!(Digest::parse("-balls:0123456789abcdef0123456789abcdef").is_err());
        assert!(Digest::parse("--balls:0123456789abcdef0123456789abcdef").is_err());
        assert!(Digest::parse("b_b+b+b+b+b+b.balllllls:0123456789abcdef0123456789abcdef").is_ok());
        assert!(Digest::parse("b_b+b+b++b+b.balllllls:0123456789abcdef0123456789abcdef").is_err());
        assert!(Digest::parse("balls:0123456789abcdef0123456789abcdef").is_ok());
        assert!(Digest::parse("balls:0123456789abcdef0123456789abcdeg").is_err());
        assert!(Digest::parse("balls:0123456789abcdef0123456789abcdefF").is_ok());
        assert!(Digest::parse("ball.ball.ball.balls:0123456789abcdef0123456789abcdef").is_ok());
        assert!(Digest::parse("0123456789abcdef0123456789abcdef").is_ok());
        assert!(Digest::parse(":0123456789abcdef0123456789abcdef").is_err());
        assert!(Digest::parse("balls:0123456789abcdef0123456789abcde").is_err());
        assert!(Digest::parse("b9:0123456789abcdef0123456789abcdef").is_ok());
        assert!(Digest::parse("b:0123456789abcdef0123456789abcdef").is_ok());
        assert!(Digest::parse("9:0123456789abcdef0123456789abcdef").is_err());
        assert!(Digest::parse(" balls:0123456789abcdef0123456789abcdef").is_err());
        assert!(Digest::parse("balls:0123456789abcdef0123456789abcdef ").is_err());
    }
}
