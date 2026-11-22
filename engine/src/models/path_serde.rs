//! Native-byte path serialization shared by durable commands and Inventory facts.
use crate::models::{Evidence, Path};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::PathBuf;

#[cfg(unix)]
fn to_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
fn to_bytes(path: &Path) -> Result<Vec<u8>, &'static str> {
    path.to_str()
        .map(|value| value.as_bytes().to_vec())
        .ok_or("Path cannot be represented as UTF-8 on this platform.")
}

#[cfg(unix)]
fn from_bytes(bytes: Vec<u8>) -> PathBuf {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    PathBuf::from(OsString::from_vec(bytes))
}

#[cfg(not(unix))]
fn from_bytes(bytes: Vec<u8>) -> Result<PathBuf, &'static str> {
    String::from_utf8(bytes)
        .map(PathBuf::from)
        .map_err(|_| "Persisted path is invalid UTF-8 on this platform.")
}

pub(crate) mod required {
    use super::*;
    pub fn serialize<S: Serializer>(path: &Path, serializer: S) -> Result<S::Ok, S::Error> {
        #[cfg(unix)]
        let bytes = to_bytes(path);
        #[cfg(not(unix))]
        let bytes = to_bytes(path).map_err(serde::ser::Error::custom)?;
        bytes.serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Path, D::Error> {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        #[cfg(unix)]
        {
            Ok(from_bytes(bytes))
        }
        #[cfg(not(unix))]
        {
            from_bytes(bytes).map_err(serde::de::Error::custom)
        }
    }
}
pub(crate) mod option {
    use super::*;

    pub fn serialize<S>(path: &Option<Path>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[cfg(unix)]
        let bytes = path.as_ref().map(to_bytes);
        #[cfg(not(unix))]
        let bytes = path
            .as_ref()
            .map(to_bytes)
            .transpose()
            .map_err(serde::ser::Error::custom)?;
        bytes.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Path>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = Option::<Vec<u8>>::deserialize(deserializer)?;
        #[cfg(unix)]
        let path = bytes.map(from_bytes);
        #[cfg(not(unix))]
        let path = bytes
            .map(from_bytes)
            .transpose()
            .map_err(serde::de::Error::custom)?;
        Ok(path)
    }
}

pub(crate) mod evidence {
    use super::*;

    pub fn serialize<S>(evidence: &Evidence<Path>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encoded = match evidence {
            Evidence::Known(path) => {
                #[cfg(unix)]
                {
                    Evidence::Known(to_bytes(path))
                }
                #[cfg(not(unix))]
                {
                    Evidence::Known(to_bytes(path).map_err(serde::ser::Error::custom)?)
                }
            }
            Evidence::NotCollected => Evidence::NotCollected,
            Evidence::NotApplicable => Evidence::NotApplicable,
            Evidence::Unsupported => Evidence::Unsupported,
            Evidence::Unavailable => Evidence::Unavailable,
            Evidence::PermissionDenied => Evidence::PermissionDenied,
        };
        encoded.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Evidence<Path>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = Evidence::<Vec<u8>>::deserialize(deserializer)?;
        match encoded {
            Evidence::Known(bytes) => {
                #[cfg(unix)]
                {
                    Ok(Evidence::Known(from_bytes(bytes)))
                }
                #[cfg(not(unix))]
                {
                    Ok(Evidence::Known(
                        from_bytes(bytes).map_err(serde::de::Error::custom)?,
                    ))
                }
            }
            Evidence::NotCollected => Ok(Evidence::NotCollected),
            Evidence::NotApplicable => Ok(Evidence::NotApplicable),
            Evidence::Unsupported => Ok(Evidence::Unsupported),
            Evidence::Unavailable => Ok(Evidence::Unavailable),
            Evidence::PermissionDenied => Ok(Evidence::PermissionDenied),
        }
    }
}
