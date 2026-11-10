//! Golden field projections fixed from the public contract, independent of HTTP builders.
use crate::providers::google::*;

#[test]
fn googleabout_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"user":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"storageQuota":{"limit":"value","usage":"value","usageInDrive":"value","usageInDriveTrash":"value"},"importFormats":{"property":["value"]},"exportFormats":{"property":["value"]},"maxImportSizes":{"property":"value"},"appInstalled":false,"folderColorPalette":["value"]}"#).unwrap();
    let value: GoogleAbout = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("user");
    let value: GoogleAbout = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("user")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("storageQuota");
    let value: GoogleAbout = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("storageQuota")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("importFormats");
    let value: GoogleAbout = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("importFormats")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("exportFormats");
    let value: GoogleAbout = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("exportFormats")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("maxImportSizes");
    let value: GoogleAbout = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("maxImportSizes")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("appInstalled");
    let value: GoogleAbout = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("appInstalled")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("folderColorPalette");
    let value: GoogleAbout = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("folderColorPalette")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleuser_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}"#).unwrap();
    let value: GoogleUser = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("displayName");
    let value: GoogleUser = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("displayName")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("emailAddress");
    let value: GoogleUser = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("emailAddress")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("photoLink");
    let value: GoogleUser = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("photoLink")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("permissionId");
    let value: GoogleUser = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("permissionId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("me");
    let value: GoogleUser = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("me")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlestoragequota_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value = serde_json::from_str(
        r#"{"limit":"value","usage":"value","usageInDrive":"value","usageInDriveTrash":"value"}"#,
    )
    .unwrap();
    let value: GoogleStorageQuota = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("limit");
    let value: GoogleStorageQuota = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("limit")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("usage");
    let value: GoogleStorageQuota = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("usage")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("usageInDrive");
    let value: GoogleStorageQuota = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("usageInDrive")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("usageInDriveTrash");
    let value: GoogleStorageQuota = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("usageInDriveTrash")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlefile_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"id":"value","name":"value","mimeType":"value","parents":["value"],"description":"value","trashed":false,"explicitlyTrashed":false,"trashedTime":"value","version":"value","createdTime":"value","modifiedTime":"value","modifiedByMeTime":"value","viewedByMeTime":"value","size":"value","quotaBytesUsed":"value","md5Checksum":"value","sha1Checksum":"value","sha256Checksum":"value","headRevisionId":"value","driveId":"value","resourceKey":"value","webViewLink":"value","webContentLink":"value","thumbnailLink":"value","iconLink":"value","fileExtension":"value","fullFileExtension":"value","originalFilename":"value","shortcutDetails":{"targetId":"value","targetMimeType":"value","targetResourceKey":"value"},"capabilities":{"canEdit":false,"canCopy":false,"canDownload":false,"canDelete":false,"canRename":false,"canMoveItemWithinDrive":false,"canMoveItemOutOfDrive":false,"canAddChildren":false,"canListChildren":false,"canShare":false,"canComment":false,"canReadRevisions":false,"canChangeCopyRequiresWriterPermission":false},"permissions":[{"id":"value","type":"value","role":"value","emailAddress":"value","domain":"value","allowFileDiscovery":false,"expirationTime":"value","displayName":"value","photoLink":"value","deleted":false,"pendingOwner":false,"permissionDetails":[{"permissionType":"value","role":"value","inherited":false,"inheritedFrom":"value"}],"view":"value"}],"owners":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}],"lastModifyingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"sharingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"shared":false,"ownedByMe":false,"modifiedByMe":false,"starred":false,"folderColorRgb":"value","properties":{"property":"value"},"appProperties":{"property":"value"},"contentHints":{"indexableText":"value","thumbnail":{"image":"value","mimeType":"value"}},"imageMediaMetadata":{"width":2,"height":2,"rotation":2,"time":"value","cameraMake":"value","cameraModel":"value","exposureTime":1.5,"aperture":1.5,"isoSpeed":2,"location":{"latitude":1.5,"longitude":1.5,"altitude":1.5}},"videoMediaMetadata":{"width":2,"height":2,"durationMillis":"value"},"labelInfo":{"labels":[{"id":"value","revisionId":"value","fields":{"property":{"valueType":"value","text":["value"],"integer":["value"],"dateString":["value"],"selection":["value"],"user":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}]}}}]},"contentRestrictions":[{"readOnly":false,"reason":"value","restrictingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"restrictionTime":"value","ownerRestricted":false,"systemRestricted":false}],"downloadRestrictions":{"itemDownloadRestriction":{"restrictedForReaders":false,"restrictedForWriters":false},"effectiveDownloadRestrictionWithContext":{"restrictedForReaders":false,"restrictedForWriters":false}},"clientEncryptionDetails":{"encryptionState":"value","decryptionMetadata":{"wrappedKey":"value","kaclsId":"value"}},"copyRequiresWriterPermission":false,"writersCanShare":false,"hasThumbnail":false,"spaces":["value"],"permissionIds":["value"],"teamDriveId":"value","inheritedPermissionsDisabled":false}"#).unwrap();
    let value: GoogleFile = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("id");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("name");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("name")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("mimeType");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("mimeType")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("parents");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("parents")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("description");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("description")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("trashed");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("trashed")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("explicitlyTrashed");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("explicitlyTrashed")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("trashedTime");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("trashedTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("version");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("version")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("createdTime");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("createdTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("modifiedTime");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("modifiedTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("modifiedByMeTime");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("modifiedByMeTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("viewedByMeTime");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("viewedByMeTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("size");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("size")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("quotaBytesUsed");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("quotaBytesUsed")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("md5Checksum");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("md5Checksum")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("sha1Checksum");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("sha1Checksum")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("sha256Checksum");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("sha256Checksum")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("headRevisionId");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("headRevisionId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("driveId");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("driveId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("resourceKey");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("resourceKey")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("webViewLink");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("webViewLink")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("webContentLink");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("webContentLink")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("thumbnailLink");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("thumbnailLink")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("iconLink");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("iconLink")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fileExtension");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fileExtension")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fullFileExtension");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fullFileExtension")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("originalFilename");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("originalFilename")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("shortcutDetails");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("shortcutDetails")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("capabilities");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("capabilities")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("permissions");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("permissions")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("owners");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("owners")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("lastModifyingUser");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("lastModifyingUser")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("sharingUser");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("sharingUser")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("shared");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("shared")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("ownedByMe");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("ownedByMe")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("modifiedByMe");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("modifiedByMe")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("starred");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("starred")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("folderColorRgb");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("folderColorRgb")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("properties");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("properties")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("appProperties");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("appProperties")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("contentHints");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("contentHints")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("imageMediaMetadata");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("imageMediaMetadata")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("videoMediaMetadata");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("videoMediaMetadata")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("labelInfo");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("labelInfo")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("contentRestrictions");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("contentRestrictions")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("downloadRestrictions");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("downloadRestrictions")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("clientEncryptionDetails");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("clientEncryptionDetails")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("copyRequiresWriterPermission");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("copyRequiresWriterPermission")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("writersCanShare");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("writersCanShare")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("hasThumbnail");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("hasThumbnail")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("spaces");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("spaces")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("permissionIds");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("permissionIds")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("teamDriveId");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("teamDriveId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("inheritedPermissionsDisabled");
    let value: GoogleFile = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("inheritedPermissionsDisabled")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleshortcutdetails_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value = serde_json::from_str(
        r#"{"targetId":"value","targetMimeType":"value","targetResourceKey":"value"}"#,
    )
    .unwrap();
    let value: GoogleShortcutDetails = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("targetId");
    let value: GoogleShortcutDetails = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("targetId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("targetMimeType");
    let value: GoogleShortcutDetails = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("targetMimeType")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("targetResourceKey");
    let value: GoogleShortcutDetails = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("targetResourceKey")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlefilecapabilities_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"canEdit":false,"canCopy":false,"canDownload":false,"canDelete":false,"canRename":false,"canMoveItemWithinDrive":false,"canMoveItemOutOfDrive":false,"canAddChildren":false,"canListChildren":false,"canShare":false,"canComment":false,"canReadRevisions":false,"canChangeCopyRequiresWriterPermission":false}"#).unwrap();
    let value: GoogleFileCapabilities = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canEdit");
    let value: GoogleFileCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canEdit")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canCopy");
    let value: GoogleFileCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canCopy")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canDownload");
    let value: GoogleFileCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canDownload")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canDelete");
    let value: GoogleFileCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canDelete")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canRename");
    let value: GoogleFileCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canRename")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("canMoveItemWithinDrive");
    let value: GoogleFileCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canMoveItemWithinDrive")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("canMoveItemOutOfDrive");
    let value: GoogleFileCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canMoveItemOutOfDrive")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canAddChildren");
    let value: GoogleFileCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canAddChildren")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canListChildren");
    let value: GoogleFileCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canListChildren")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canShare");
    let value: GoogleFileCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canShare")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canComment");
    let value: GoogleFileCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canComment")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canReadRevisions");
    let value: GoogleFileCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canReadRevisions")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("canChangeCopyRequiresWriterPermission");
    let value: GoogleFileCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canChangeCopyRequiresWriterPermission")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlecontenthints_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value = serde_json::from_str(
        r#"{"indexableText":"value","thumbnail":{"image":"value","mimeType":"value"}}"#,
    )
    .unwrap();
    let value: GoogleContentHints = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("indexableText");
    let value: GoogleContentHints = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("indexableText")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("thumbnail");
    let value: GoogleContentHints = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("thumbnail")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlethumbnail_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"image":"value","mimeType":"value"}"#).unwrap();
    let value: GoogleThumbnail = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("image");
    let value: GoogleThumbnail = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("image")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("mimeType");
    let value: GoogleThumbnail = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("mimeType")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleimagemediametadata_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"width":2,"height":2,"rotation":2,"time":"value","cameraMake":"value","cameraModel":"value","exposureTime":1.5,"aperture":1.5,"isoSpeed":2,"location":{"latitude":1.5,"longitude":1.5,"altitude":1.5}}"#).unwrap();
    let value: GoogleImageMediaMetadata = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("width");
    let value: GoogleImageMediaMetadata = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("width")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("height");
    let value: GoogleImageMediaMetadata = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("height")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("rotation");
    let value: GoogleImageMediaMetadata = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("rotation")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("time");
    let value: GoogleImageMediaMetadata = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("time")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("cameraMake");
    let value: GoogleImageMediaMetadata = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("cameraMake")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("cameraModel");
    let value: GoogleImageMediaMetadata = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("cameraModel")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("exposureTime");
    let value: GoogleImageMediaMetadata = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("exposureTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("aperture");
    let value: GoogleImageMediaMetadata = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("aperture")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("isoSpeed");
    let value: GoogleImageMediaMetadata = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("isoSpeed")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("location");
    let value: GoogleImageMediaMetadata = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("location")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlegeolocation_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"latitude":1.5,"longitude":1.5,"altitude":1.5}"#).unwrap();
    let value: GoogleGeoLocation = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("latitude");
    let value: GoogleGeoLocation = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("latitude")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("longitude");
    let value: GoogleGeoLocation = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("longitude")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("altitude");
    let value: GoogleGeoLocation = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("altitude")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlevideomediametadata_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"width":2,"height":2,"durationMillis":"value"}"#).unwrap();
    let value: GoogleVideoMediaMetadata = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("width");
    let value: GoogleVideoMediaMetadata = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("width")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("height");
    let value: GoogleVideoMediaMetadata = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("height")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("durationMillis");
    let value: GoogleVideoMediaMetadata = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("durationMillis")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlecontentrestriction_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"readOnly":false,"reason":"value","restrictingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"restrictionTime":"value","ownerRestricted":false,"systemRestricted":false}"#).unwrap();
    let value: GoogleContentRestriction = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("readOnly");
    let value: GoogleContentRestriction = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("readOnly")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("reason");
    let value: GoogleContentRestriction = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("reason")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("restrictingUser");
    let value: GoogleContentRestriction = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("restrictingUser")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("restrictionTime");
    let value: GoogleContentRestriction = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("restrictionTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("ownerRestricted");
    let value: GoogleContentRestriction = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("ownerRestricted")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("systemRestricted");
    let value: GoogleContentRestriction = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("systemRestricted")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googledownloadrestrictions_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"itemDownloadRestriction":{"restrictedForReaders":false,"restrictedForWriters":false},"effectiveDownloadRestrictionWithContext":{"restrictedForReaders":false,"restrictedForWriters":false}}"#).unwrap();
    let value: GoogleDownloadRestrictions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("itemDownloadRestriction");
    let value: GoogleDownloadRestrictions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("itemDownloadRestriction")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("effectiveDownloadRestrictionWithContext");
    let value: GoogleDownloadRestrictions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("effectiveDownloadRestrictionWithContext")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googledownloadrestriction_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"restrictedForReaders":false,"restrictedForWriters":false}"#)
            .unwrap();
    let value: GoogleDownloadRestriction = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("restrictedForReaders");
    let value: GoogleDownloadRestriction = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("restrictedForReaders")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("restrictedForWriters");
    let value: GoogleDownloadRestriction = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("restrictedForWriters")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleclientencryptiondetails_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"encryptionState":"value","decryptionMetadata":{"wrappedKey":"value","kaclsId":"value"}}"#).unwrap();
    let value: GoogleClientEncryptionDetails = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("encryptionState");
    let value: GoogleClientEncryptionDetails = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("encryptionState")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("decryptionMetadata");
    let value: GoogleClientEncryptionDetails = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("decryptionMetadata")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googledecryptionmetadata_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"wrappedKey":"value","kaclsId":"value"}"#).unwrap();
    let value: GoogleDecryptionMetadata = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("wrappedKey");
    let value: GoogleDecryptionMetadata = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("wrappedKey")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("kaclsId");
    let value: GoogleDecryptionMetadata = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("kaclsId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlelabelinfo_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"labels":[{"id":"value","revisionId":"value","fields":{"property":{"valueType":"value","text":["value"],"integer":["value"],"dateString":["value"],"selection":["value"],"user":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}]}}}]}"#).unwrap();
    let value: GoogleLabelInfo = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("labels");
    let value: GoogleLabelInfo = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("labels")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlelabel_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"id":"value","revisionId":"value","fields":{"property":{"valueType":"value","text":["value"],"integer":["value"],"dateString":["value"],"selection":["value"],"user":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}]}}}"#).unwrap();
    let value: GoogleLabel = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("id");
    let value: GoogleLabel = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("revisionId");
    let value: GoogleLabel = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("revisionId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: GoogleLabel = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlelabelfield_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"valueType":"value","text":["value"],"integer":["value"],"dateString":["value"],"selection":["value"],"user":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}]}"#).unwrap();
    let value: GoogleLabelField = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("valueType");
    let value: GoogleLabelField = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("valueType")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("text");
    let value: GoogleLabelField = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("text")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("integer");
    let value: GoogleLabelField = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("integer")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("dateString");
    let value: GoogleLabelField = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("dateString")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("selection");
    let value: GoogleLabelField = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("selection")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("user");
    let value: GoogleLabelField = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("user")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlefilelist_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"files":[{"id":"value","name":"value","mimeType":"value","parents":["value"],"description":"value","trashed":false,"explicitlyTrashed":false,"trashedTime":"value","version":"value","createdTime":"value","modifiedTime":"value","modifiedByMeTime":"value","viewedByMeTime":"value","size":"value","quotaBytesUsed":"value","md5Checksum":"value","sha1Checksum":"value","sha256Checksum":"value","headRevisionId":"value","driveId":"value","resourceKey":"value","webViewLink":"value","webContentLink":"value","thumbnailLink":"value","iconLink":"value","fileExtension":"value","fullFileExtension":"value","originalFilename":"value","shortcutDetails":{"targetId":"value","targetMimeType":"value","targetResourceKey":"value"},"capabilities":{"canEdit":false,"canCopy":false,"canDownload":false,"canDelete":false,"canRename":false,"canMoveItemWithinDrive":false,"canMoveItemOutOfDrive":false,"canAddChildren":false,"canListChildren":false,"canShare":false,"canComment":false,"canReadRevisions":false,"canChangeCopyRequiresWriterPermission":false},"permissions":[{"id":"value","type":"value","role":"value","emailAddress":"value","domain":"value","allowFileDiscovery":false,"expirationTime":"value","displayName":"value","photoLink":"value","deleted":false,"pendingOwner":false,"permissionDetails":[{"permissionType":"value","role":"value","inherited":false,"inheritedFrom":"value"}],"view":"value"}],"owners":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}],"lastModifyingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"sharingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"shared":false,"ownedByMe":false,"modifiedByMe":false,"starred":false,"folderColorRgb":"value","properties":{"property":"value"},"appProperties":{"property":"value"},"contentHints":{"indexableText":"value","thumbnail":{"image":"value","mimeType":"value"}},"imageMediaMetadata":{"width":2,"height":2,"rotation":2,"time":"value","cameraMake":"value","cameraModel":"value","exposureTime":1.5,"aperture":1.5,"isoSpeed":2,"location":{"latitude":1.5,"longitude":1.5,"altitude":1.5}},"videoMediaMetadata":{"width":2,"height":2,"durationMillis":"value"},"labelInfo":{"labels":[{"id":"value","revisionId":"value","fields":{"property":{"valueType":"value","text":["value"],"integer":["value"],"dateString":["value"],"selection":["value"],"user":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}]}}}]},"contentRestrictions":[{"readOnly":false,"reason":"value","restrictingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"restrictionTime":"value","ownerRestricted":false,"systemRestricted":false}],"downloadRestrictions":{"itemDownloadRestriction":{"restrictedForReaders":false,"restrictedForWriters":false},"effectiveDownloadRestrictionWithContext":{"restrictedForReaders":false,"restrictedForWriters":false}},"clientEncryptionDetails":{"encryptionState":"value","decryptionMetadata":{"wrappedKey":"value","kaclsId":"value"}},"copyRequiresWriterPermission":false,"writersCanShare":false,"hasThumbnail":false,"spaces":["value"],"permissionIds":["value"],"teamDriveId":"value","inheritedPermissionsDisabled":false}],"nextPageToken":"value","incompleteSearch":false}"#).unwrap();
    let value: GoogleFileList = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("files");
    let value: GoogleFileList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("files")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Array(vec![])
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("nextPageToken");
    let value: GoogleFileList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("nextPageToken")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("incompleteSearch");
    let value: GoogleFileList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("incompleteSearch")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlechange_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"fileId":"value","file":{"id":"value","name":"value","mimeType":"value","parents":["value"],"description":"value","trashed":false,"explicitlyTrashed":false,"trashedTime":"value","version":"value","createdTime":"value","modifiedTime":"value","modifiedByMeTime":"value","viewedByMeTime":"value","size":"value","quotaBytesUsed":"value","md5Checksum":"value","sha1Checksum":"value","sha256Checksum":"value","headRevisionId":"value","driveId":"value","resourceKey":"value","webViewLink":"value","webContentLink":"value","thumbnailLink":"value","iconLink":"value","fileExtension":"value","fullFileExtension":"value","originalFilename":"value","shortcutDetails":{"targetId":"value","targetMimeType":"value","targetResourceKey":"value"},"capabilities":{"canEdit":false,"canCopy":false,"canDownload":false,"canDelete":false,"canRename":false,"canMoveItemWithinDrive":false,"canMoveItemOutOfDrive":false,"canAddChildren":false,"canListChildren":false,"canShare":false,"canComment":false,"canReadRevisions":false,"canChangeCopyRequiresWriterPermission":false},"permissions":[{"id":"value","type":"value","role":"value","emailAddress":"value","domain":"value","allowFileDiscovery":false,"expirationTime":"value","displayName":"value","photoLink":"value","deleted":false,"pendingOwner":false,"permissionDetails":[{"permissionType":"value","role":"value","inherited":false,"inheritedFrom":"value"}],"view":"value"}],"owners":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}],"lastModifyingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"sharingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"shared":false,"ownedByMe":false,"modifiedByMe":false,"starred":false,"folderColorRgb":"value","properties":{"property":"value"},"appProperties":{"property":"value"},"contentHints":{"indexableText":"value","thumbnail":{"image":"value","mimeType":"value"}},"imageMediaMetadata":{"width":2,"height":2,"rotation":2,"time":"value","cameraMake":"value","cameraModel":"value","exposureTime":1.5,"aperture":1.5,"isoSpeed":2,"location":{"latitude":1.5,"longitude":1.5,"altitude":1.5}},"videoMediaMetadata":{"width":2,"height":2,"durationMillis":"value"},"labelInfo":{"labels":[{"id":"value","revisionId":"value","fields":{"property":{"valueType":"value","text":["value"],"integer":["value"],"dateString":["value"],"selection":["value"],"user":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}]}}}]},"contentRestrictions":[{"readOnly":false,"reason":"value","restrictingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"restrictionTime":"value","ownerRestricted":false,"systemRestricted":false}],"downloadRestrictions":{"itemDownloadRestriction":{"restrictedForReaders":false,"restrictedForWriters":false},"effectiveDownloadRestrictionWithContext":{"restrictedForReaders":false,"restrictedForWriters":false}},"clientEncryptionDetails":{"encryptionState":"value","decryptionMetadata":{"wrappedKey":"value","kaclsId":"value"}},"copyRequiresWriterPermission":false,"writersCanShare":false,"hasThumbnail":false,"spaces":["value"],"permissionIds":["value"],"teamDriveId":"value","inheritedPermissionsDisabled":false},"removed":false,"time":"value","changeType":"value","driveId":"value","drive":{"id":"value","name":"value","hidden":false,"createdTime":"value","restrictions":{"copyRequiresWriterPermission":false,"domainUsersOnly":false,"driveMembersOnly":false,"adminManagedRestrictions":false,"sharingFoldersRequiresOrganizerPermission":false},"capabilities":{"canAddChildren":false,"canComment":false,"canCopy":false,"canDeleteDrive":false,"canDownload":false,"canEdit":false,"canListChildren":false,"canManageMembers":false,"canReadRevisions":false,"canRename":false,"canShare":false},"colorRgb":"value","backgroundImageLink":"value","orgUnitId":"value"},"changeId":"value"}"#).unwrap();
    let value: GoogleChange = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fileId");
    let value: GoogleChange = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fileId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("file");
    let value: GoogleChange = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("file")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("removed");
    let value: GoogleChange = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("removed")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("time");
    let value: GoogleChange = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("time")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("changeType");
    let value: GoogleChange = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("changeType")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("driveId");
    let value: GoogleChange = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("driveId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("drive");
    let value: GoogleChange = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("drive")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("changeId");
    let value: GoogleChange = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("changeId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlechangelist_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"changes":[{"fileId":"value","file":{"id":"value","name":"value","mimeType":"value","parents":["value"],"description":"value","trashed":false,"explicitlyTrashed":false,"trashedTime":"value","version":"value","createdTime":"value","modifiedTime":"value","modifiedByMeTime":"value","viewedByMeTime":"value","size":"value","quotaBytesUsed":"value","md5Checksum":"value","sha1Checksum":"value","sha256Checksum":"value","headRevisionId":"value","driveId":"value","resourceKey":"value","webViewLink":"value","webContentLink":"value","thumbnailLink":"value","iconLink":"value","fileExtension":"value","fullFileExtension":"value","originalFilename":"value","shortcutDetails":{"targetId":"value","targetMimeType":"value","targetResourceKey":"value"},"capabilities":{"canEdit":false,"canCopy":false,"canDownload":false,"canDelete":false,"canRename":false,"canMoveItemWithinDrive":false,"canMoveItemOutOfDrive":false,"canAddChildren":false,"canListChildren":false,"canShare":false,"canComment":false,"canReadRevisions":false,"canChangeCopyRequiresWriterPermission":false},"permissions":[{"id":"value","type":"value","role":"value","emailAddress":"value","domain":"value","allowFileDiscovery":false,"expirationTime":"value","displayName":"value","photoLink":"value","deleted":false,"pendingOwner":false,"permissionDetails":[{"permissionType":"value","role":"value","inherited":false,"inheritedFrom":"value"}],"view":"value"}],"owners":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}],"lastModifyingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"sharingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"shared":false,"ownedByMe":false,"modifiedByMe":false,"starred":false,"folderColorRgb":"value","properties":{"property":"value"},"appProperties":{"property":"value"},"contentHints":{"indexableText":"value","thumbnail":{"image":"value","mimeType":"value"}},"imageMediaMetadata":{"width":2,"height":2,"rotation":2,"time":"value","cameraMake":"value","cameraModel":"value","exposureTime":1.5,"aperture":1.5,"isoSpeed":2,"location":{"latitude":1.5,"longitude":1.5,"altitude":1.5}},"videoMediaMetadata":{"width":2,"height":2,"durationMillis":"value"},"labelInfo":{"labels":[{"id":"value","revisionId":"value","fields":{"property":{"valueType":"value","text":["value"],"integer":["value"],"dateString":["value"],"selection":["value"],"user":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}]}}}]},"contentRestrictions":[{"readOnly":false,"reason":"value","restrictingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"restrictionTime":"value","ownerRestricted":false,"systemRestricted":false}],"downloadRestrictions":{"itemDownloadRestriction":{"restrictedForReaders":false,"restrictedForWriters":false},"effectiveDownloadRestrictionWithContext":{"restrictedForReaders":false,"restrictedForWriters":false}},"clientEncryptionDetails":{"encryptionState":"value","decryptionMetadata":{"wrappedKey":"value","kaclsId":"value"}},"copyRequiresWriterPermission":false,"writersCanShare":false,"hasThumbnail":false,"spaces":["value"],"permissionIds":["value"],"teamDriveId":"value","inheritedPermissionsDisabled":false},"removed":false,"time":"value","changeType":"value","driveId":"value","drive":{"id":"value","name":"value","hidden":false,"createdTime":"value","restrictions":{"copyRequiresWriterPermission":false,"domainUsersOnly":false,"driveMembersOnly":false,"adminManagedRestrictions":false,"sharingFoldersRequiresOrganizerPermission":false},"capabilities":{"canAddChildren":false,"canComment":false,"canCopy":false,"canDeleteDrive":false,"canDownload":false,"canEdit":false,"canListChildren":false,"canManageMembers":false,"canReadRevisions":false,"canRename":false,"canShare":false},"colorRgb":"value","backgroundImageLink":"value","orgUnitId":"value"},"changeId":"value"}],"nextPageToken":"value","newStartPageToken":"value"}"#).unwrap();
    let value: GoogleChangeList = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("changes");
    let value: GoogleChangeList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("changes")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Array(vec![])
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("nextPageToken");
    let value: GoogleChangeList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("nextPageToken")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("newStartPageToken");
    let value: GoogleChangeList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("newStartPageToken")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlestartpagetoken_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"startPageToken":"value"}"#).unwrap();
    let value: GoogleStartPageToken = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("startPageToken");
    assert!(serde_json::from_value::<GoogleStartPageToken>(omitted).is_err());
}

#[test]
fn googledrive_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"id":"value","name":"value","hidden":false,"createdTime":"value","restrictions":{"copyRequiresWriterPermission":false,"domainUsersOnly":false,"driveMembersOnly":false,"adminManagedRestrictions":false,"sharingFoldersRequiresOrganizerPermission":false},"capabilities":{"canAddChildren":false,"canComment":false,"canCopy":false,"canDeleteDrive":false,"canDownload":false,"canEdit":false,"canListChildren":false,"canManageMembers":false,"canReadRevisions":false,"canRename":false,"canShare":false},"colorRgb":"value","backgroundImageLink":"value","orgUnitId":"value"}"#).unwrap();
    let value: GoogleDrive = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("id");
    let value: GoogleDrive = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("name");
    let value: GoogleDrive = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("name")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("hidden");
    let value: GoogleDrive = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("hidden")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("createdTime");
    let value: GoogleDrive = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("createdTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("restrictions");
    let value: GoogleDrive = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("restrictions")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("capabilities");
    let value: GoogleDrive = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("capabilities")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("colorRgb");
    let value: GoogleDrive = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("colorRgb")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("backgroundImageLink");
    let value: GoogleDrive = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("backgroundImageLink")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("orgUnitId");
    let value: GoogleDrive = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("orgUnitId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googledriverestrictions_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"copyRequiresWriterPermission":false,"domainUsersOnly":false,"driveMembersOnly":false,"adminManagedRestrictions":false,"sharingFoldersRequiresOrganizerPermission":false}"#).unwrap();
    let value: GoogleDriveRestrictions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("copyRequiresWriterPermission");
    let value: GoogleDriveRestrictions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("copyRequiresWriterPermission")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("domainUsersOnly");
    let value: GoogleDriveRestrictions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("domainUsersOnly")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("driveMembersOnly");
    let value: GoogleDriveRestrictions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("driveMembersOnly")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("adminManagedRestrictions");
    let value: GoogleDriveRestrictions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("adminManagedRestrictions")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("sharingFoldersRequiresOrganizerPermission");
    let value: GoogleDriveRestrictions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("sharingFoldersRequiresOrganizerPermission")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googledrivecapabilities_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"canAddChildren":false,"canComment":false,"canCopy":false,"canDeleteDrive":false,"canDownload":false,"canEdit":false,"canListChildren":false,"canManageMembers":false,"canReadRevisions":false,"canRename":false,"canShare":false}"#).unwrap();
    let value: GoogleDriveCapabilities = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canAddChildren");
    let value: GoogleDriveCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canAddChildren")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canComment");
    let value: GoogleDriveCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canComment")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canCopy");
    let value: GoogleDriveCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canCopy")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canDeleteDrive");
    let value: GoogleDriveCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canDeleteDrive")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canDownload");
    let value: GoogleDriveCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canDownload")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canEdit");
    let value: GoogleDriveCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canEdit")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canListChildren");
    let value: GoogleDriveCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canListChildren")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canManageMembers");
    let value: GoogleDriveCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canManageMembers")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canReadRevisions");
    let value: GoogleDriveCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canReadRevisions")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canRename");
    let value: GoogleDriveCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canRename")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("canShare");
    let value: GoogleDriveCapabilities = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("canShare")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlepermission_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"id":"value","type":"value","role":"value","emailAddress":"value","domain":"value","allowFileDiscovery":false,"expirationTime":"value","displayName":"value","photoLink":"value","deleted":false,"pendingOwner":false,"permissionDetails":[{"permissionType":"value","role":"value","inherited":false,"inheritedFrom":"value"}],"view":"value"}"#).unwrap();
    let value: GooglePermission = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("id");
    let value: GooglePermission = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("type");
    let value: GooglePermission = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("type")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("role");
    let value: GooglePermission = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("role")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("emailAddress");
    let value: GooglePermission = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("emailAddress")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("domain");
    let value: GooglePermission = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("domain")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("allowFileDiscovery");
    let value: GooglePermission = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("allowFileDiscovery")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("expirationTime");
    let value: GooglePermission = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("expirationTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("displayName");
    let value: GooglePermission = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("displayName")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("photoLink");
    let value: GooglePermission = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("photoLink")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("deleted");
    let value: GooglePermission = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("deleted")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("pendingOwner");
    let value: GooglePermission = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("pendingOwner")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("permissionDetails");
    let value: GooglePermission = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("permissionDetails")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("view");
    let value: GooglePermission = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("view")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlepermissiondetails_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value = serde_json::from_str(
        r#"{"permissionType":"value","role":"value","inherited":false,"inheritedFrom":"value"}"#,
    )
    .unwrap();
    let value: GooglePermissionDetails = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("permissionType");
    let value: GooglePermissionDetails = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("permissionType")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("role");
    let value: GooglePermissionDetails = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("role")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("inherited");
    let value: GooglePermissionDetails = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("inherited")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("inheritedFrom");
    let value: GooglePermissionDetails = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("inheritedFrom")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlerevision_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"id":"value","mimeType":"value","modifiedTime":"value","keepForever":false,"size":"value","md5Checksum":"value","published":false,"publishAuto":false,"publishedOutsideDomain":false,"originalFilename":"value","lastModifyingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"exportLinks":{"property":"value"}}"#).unwrap();
    let value: GoogleRevision = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("id");
    let value: GoogleRevision = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("mimeType");
    let value: GoogleRevision = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("mimeType")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("modifiedTime");
    let value: GoogleRevision = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("modifiedTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("keepForever");
    let value: GoogleRevision = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("keepForever")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("size");
    let value: GoogleRevision = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("size")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("md5Checksum");
    let value: GoogleRevision = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("md5Checksum")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("published");
    let value: GoogleRevision = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("published")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("publishAuto");
    let value: GoogleRevision = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("publishAuto")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("publishedOutsideDomain");
    let value: GoogleRevision = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("publishedOutsideDomain")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("originalFilename");
    let value: GoogleRevision = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("originalFilename")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("lastModifyingUser");
    let value: GoogleRevision = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("lastModifyingUser")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("exportLinks");
    let value: GoogleRevision = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("exportLinks")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlecomment_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"id":"value","content":"value","createdTime":"value","modifiedTime":"value","author":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"resolved":false,"deleted":false,"replies":[{"id":"value","content":"value","createdTime":"value","modifiedTime":"value","author":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"action":"value","deleted":false}],"anchor":"value","quotedFileContent":{"mimeType":"value","value":"value"}}"#).unwrap();
    let value: GoogleComment = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("id");
    let value: GoogleComment = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("content");
    let value: GoogleComment = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("content")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("createdTime");
    let value: GoogleComment = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("createdTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("modifiedTime");
    let value: GoogleComment = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("modifiedTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("author");
    let value: GoogleComment = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("author")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("resolved");
    let value: GoogleComment = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("resolved")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("deleted");
    let value: GoogleComment = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("deleted")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("replies");
    let value: GoogleComment = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("replies")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("anchor");
    let value: GoogleComment = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("anchor")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("quotedFileContent");
    let value: GoogleComment = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("quotedFileContent")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlequotedcontent_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"mimeType":"value","value":"value"}"#).unwrap();
    let value: GoogleQuotedContent = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("mimeType");
    let value: GoogleQuotedContent = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("mimeType")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("value");
    let value: GoogleQuotedContent = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("value")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlereply_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"id":"value","content":"value","createdTime":"value","modifiedTime":"value","author":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"action":"value","deleted":false}"#).unwrap();
    let value: GoogleReply = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("id");
    let value: GoogleReply = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("content");
    let value: GoogleReply = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("content")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("createdTime");
    let value: GoogleReply = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("createdTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("modifiedTime");
    let value: GoogleReply = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("modifiedTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("author");
    let value: GoogleReply = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("author")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("action");
    let value: GoogleReply = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("action")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("deleted");
    let value: GoogleReply = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("deleted")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlechannel_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"id":"value","resourceId":"value","resourceUri":"value","expiration":"value","token":"value","type":"value","address":"value","params":{"property":"value"}}"#).unwrap();
    let value: GoogleChannel = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("id");
    let value: GoogleChannel = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("resourceId");
    let value: GoogleChannel = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("resourceId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("resourceUri");
    let value: GoogleChannel = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("resourceUri")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("expiration");
    let value: GoogleChannel = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("expiration")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("token");
    let value: GoogleChannel = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("token")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("type");
    let value: GoogleChannel = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("type")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("address");
    let value: GoogleChannel = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("address")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("params");
    let value: GoogleChannel = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("params")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleapp_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"id":"value","name":"value","objectType":"value","productUrl":"value","createUrl":"value","openUrlTemplate":"value","supportsCreate":false,"supportsImport":false,"installed":false,"authorized":false,"useByDefault":false,"primaryMimeTypes":["value"],"secondaryMimeTypes":["value"]}"#).unwrap();
    let value: GoogleApp = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("id");
    let value: GoogleApp = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("name");
    let value: GoogleApp = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("name")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("objectType");
    let value: GoogleApp = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("objectType")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("productUrl");
    let value: GoogleApp = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("productUrl")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("createUrl");
    let value: GoogleApp = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("createUrl")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("openUrlTemplate");
    let value: GoogleApp = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("openUrlTemplate")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("supportsCreate");
    let value: GoogleApp = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("supportsCreate")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("supportsImport");
    let value: GoogleApp = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("supportsImport")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("installed");
    let value: GoogleApp = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("installed")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("authorized");
    let value: GoogleApp = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("authorized")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("useByDefault");
    let value: GoogleApp = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("useByDefault")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("primaryMimeTypes");
    let value: GoogleApp = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("primaryMimeTypes")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("secondaryMimeTypes");
    let value: GoogleApp = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("secondaryMimeTypes")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleaccessproposal_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"fileId":"value","proposalId":"value","requesterEmailAddress":"value","recipientEmailAddress":"value","rolesAndViews":[{"role":"value","view":"value"}],"requestMessage":"value","createTime":"value"}"#).unwrap();
    let value: GoogleAccessProposal = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fileId");
    let value: GoogleAccessProposal = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fileId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("proposalId");
    let value: GoogleAccessProposal = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("proposalId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("requesterEmailAddress");
    let value: GoogleAccessProposal = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("requesterEmailAddress")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("recipientEmailAddress");
    let value: GoogleAccessProposal = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("recipientEmailAddress")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("rolesAndViews");
    let value: GoogleAccessProposal = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("rolesAndViews")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("requestMessage");
    let value: GoogleAccessProposal = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("requestMessage")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("createTime");
    let value: GoogleAccessProposal = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("createTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleroleandview_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"role":"value","view":"value"}"#).unwrap();
    let value: GoogleRoleAndView = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("role");
    let value: GoogleRoleAndView = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("role")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("view");
    let value: GoogleRoleAndView = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("view")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleapproval_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"approvalId":"value","targetFileId":"value","createTime":"value","modifyTime":"value","completeTime":"value","dueTime":"value","status":"value","initiator":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"reviewerResponses":[{"reviewer":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"response":"value"}],"fileContentChangeBehavior":"value"}"#).unwrap();
    let value: GoogleApproval = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("approvalId");
    let value: GoogleApproval = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("approvalId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("targetFileId");
    let value: GoogleApproval = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("targetFileId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("createTime");
    let value: GoogleApproval = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("createTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("modifyTime");
    let value: GoogleApproval = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("modifyTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("completeTime");
    let value: GoogleApproval = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("completeTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("dueTime");
    let value: GoogleApproval = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("dueTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("status");
    let value: GoogleApproval = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("status")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("initiator");
    let value: GoogleApproval = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("initiator")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("reviewerResponses");
    let value: GoogleApproval = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("reviewerResponses")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("fileContentChangeBehavior");
    let value: GoogleApproval = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fileContentChangeBehavior")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleapprovalreviewerresponse_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"reviewer":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"response":"value"}"#).unwrap();
    let value: GoogleApprovalReviewerResponse = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("reviewer");
    let value: GoogleApprovalReviewerResponse = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("reviewer")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("response");
    let value: GoogleApprovalReviewerResponse = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("response")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleoperation_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"name":"value","done":false,"metadata":{"state":"value","progressPercent":2},"response":{"downloadUri":"value","partialDownloadAllowed":false},"error":{"code":2,"message":"value","details":[{"@type":"value","reason":"value","domain":"value","metadata":{"property":"value"}}]}}"#).unwrap();
    let value: GoogleOperation = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("name");
    let value: GoogleOperation = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("name")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("done");
    let value: GoogleOperation = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("done")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("metadata");
    let value: GoogleOperation = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("metadata")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("response");
    let value: GoogleOperation = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("response")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("error");
    let value: GoogleOperation = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("error")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleoperationmetadata_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"state":"value","progressPercent":2}"#).unwrap();
    let value: GoogleOperationMetadata = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("state");
    let value: GoogleOperationMetadata = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("state")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("progressPercent");
    let value: GoogleOperationMetadata = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("progressPercent")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlerpcstatus_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"code":2,"message":"value","details":[{"@type":"value","reason":"value","domain":"value","metadata":{"property":"value"}}]}"#).unwrap();
    let value: GoogleRpcStatus = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("code");
    let value: GoogleRpcStatus = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("code")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("message");
    let value: GoogleRpcStatus = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("message")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("details");
    let value: GoogleRpcStatus = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("details")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlerpcdetail_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value = serde_json::from_str(
        r#"{"@type":"value","reason":"value","domain":"value","metadata":{"property":"value"}}"#,
    )
    .unwrap();
    let value: GoogleRpcDetail = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("@type");
    let value: GoogleRpcDetail = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("@type")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("reason");
    let value: GoogleRpcDetail = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("reason")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("domain");
    let value: GoogleRpcDetail = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("domain")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("metadata");
    let value: GoogleRpcDetail = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("metadata")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googledownloadfileresponse_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"downloadUri":"value","partialDownloadAllowed":false}"#).unwrap();
    let value: GoogleDownloadFileResponse = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("downloadUri");
    let value: GoogleDownloadFileResponse = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("downloadUri")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("partialDownloadAllowed");
    let value: GoogleDownloadFileResponse = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("partialDownloadAllowed")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlecsetoken_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value = serde_json::from_str(
        r#"{"fileId":"value","currentKaclsId":"value","currentKaclsName":"value","jwt":"value"}"#,
    )
    .unwrap();
    let value: GoogleCseToken = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fileId");
    let value: GoogleCseToken = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fileId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("currentKaclsId");
    let value: GoogleCseToken = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("currentKaclsId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("currentKaclsName");
    let value: GoogleCseToken = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("currentKaclsName")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("jwt");
    let value: GoogleCseToken = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("jwt")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlegeneratedids_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"ids":["value"],"space":"value"}"#).unwrap();
    let value: GoogleGeneratedIds = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("ids");
    let value: GoogleGeneratedIds = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("ids")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("space");
    let value: GoogleGeneratedIds = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("space")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlemodifylabelsresponse_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"modifiedLabels":[{"id":"value","revisionId":"value","fields":{"property":{"valueType":"value","text":["value"],"integer":["value"],"dateString":["value"],"selection":["value"],"user":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}]}}}]}"#).unwrap();
    let value: GoogleModifyLabelsResponse = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("modifiedLabels");
    let value: GoogleModifyLabelsResponse = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("modifiedLabels")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlefilewrite_partial_write_fields() {
    let value = serde_json::json!("value");
    let request = GoogleFileWrite {
        name: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"name":value})
    );
    let request = GoogleFileWrite {
        name: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"name":null})
    );
    let value = serde_json::json!("value");
    let request = GoogleFileWrite {
        mime_type: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"mimeType":value})
    );
    let request = GoogleFileWrite {
        mime_type: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"mimeType":null})
    );
    let value = serde_json::json!("value");
    let request = GoogleFileWrite {
        description: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"description":value})
    );
    let request = GoogleFileWrite {
        description: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"description":null})
    );
    let value = serde_json::json!(false);
    let request = GoogleFileWrite {
        trashed: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"trashed":value})
    );
    let request = GoogleFileWrite {
        trashed: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"trashed":null})
    );
    let value = serde_json::json!(["value"]);
    let request = GoogleFileWrite {
        parents: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"parents":value})
    );
    let request = GoogleFileWrite {
        parents: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"parents":null})
    );
    let value = serde_json::json!(false);
    let request = GoogleFileWrite {
        starred: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"starred":value})
    );
    let request = GoogleFileWrite {
        starred: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"starred":null})
    );
    let value = serde_json::json!({"property": "value"});
    let request = GoogleFileWrite {
        properties: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"properties":value})
    );
    let request = GoogleFileWrite {
        properties: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"properties":null})
    );
    let value = serde_json::json!({"property": "value"});
    let request = GoogleFileWrite {
        app_properties: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"appProperties":value})
    );
    let request = GoogleFileWrite {
        app_properties: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"appProperties":null})
    );
    let value = serde_json::json!(false);
    let request = GoogleFileWrite {
        copy_requires_writer_permission: GooglePatch::Set(
            serde_json::from_value(value.clone()).unwrap(),
        ),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"copyRequiresWriterPermission":value})
    );
    let request = GoogleFileWrite {
        copy_requires_writer_permission: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"copyRequiresWriterPermission":null})
    );
    let value = serde_json::json!("value");
    let request = GoogleFileWrite {
        folder_color_rgb: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"folderColorRgb":value})
    );
    let request = GoogleFileWrite {
        folder_color_rgb: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"folderColorRgb":null})
    );
    let value = serde_json::json!("value");
    let request = GoogleFileWrite {
        modified_time: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"modifiedTime":value})
    );
    let request = GoogleFileWrite {
        modified_time: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"modifiedTime":null})
    );
}

#[test]
fn googledrivewrite_partial_write_fields() {
    let value = serde_json::json!("value");
    let request = GoogleDriveWrite {
        name: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"name":value})
    );
    let request = GoogleDriveWrite {
        name: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"name":null})
    );
    let value = serde_json::json!({"copyRequiresWriterPermission": false, "domainUsersOnly": false, "driveMembersOnly": false, "adminManagedRestrictions": false, "sharingFoldersRequiresOrganizerPermission": false});
    let request = GoogleDriveWrite {
        restrictions: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"restrictions":value})
    );
    let request = GoogleDriveWrite {
        restrictions: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"restrictions":null})
    );
    let value = serde_json::json!("value");
    let request = GoogleDriveWrite {
        color_rgb: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"colorRgb":value})
    );
    let request = GoogleDriveWrite {
        color_rgb: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"colorRgb":null})
    );
}

#[test]
fn googlepermissionwrite_partial_write_fields() {
    let value = serde_json::json!("value");
    let request = GooglePermissionWrite {
        type_: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"type":value})
    );
    let request = GooglePermissionWrite {
        type_: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"type":null})
    );
    let value = serde_json::json!("value");
    let request = GooglePermissionWrite {
        role: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"role":value})
    );
    let request = GooglePermissionWrite {
        role: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"role":null})
    );
    let value = serde_json::json!("value");
    let request = GooglePermissionWrite {
        email_address: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"emailAddress":value})
    );
    let request = GooglePermissionWrite {
        email_address: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"emailAddress":null})
    );
    let value = serde_json::json!("value");
    let request = GooglePermissionWrite {
        domain: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"domain":value})
    );
    let request = GooglePermissionWrite {
        domain: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"domain":null})
    );
    let value = serde_json::json!(false);
    let request = GooglePermissionWrite {
        allow_file_discovery: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"allowFileDiscovery":value})
    );
    let request = GooglePermissionWrite {
        allow_file_discovery: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"allowFileDiscovery":null})
    );
    let value = serde_json::json!("value");
    let request = GooglePermissionWrite {
        expiration_time: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"expirationTime":value})
    );
    let request = GooglePermissionWrite {
        expiration_time: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"expirationTime":null})
    );
    let value = serde_json::json!("value");
    let request = GooglePermissionWrite {
        view: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"view":value})
    );
    let request = GooglePermissionWrite {
        view: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"view":null})
    );
}

#[test]
fn googlecommentwrite_partial_write_fields() {
    let value = serde_json::json!("value");
    let request = GoogleCommentWrite {
        content: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"content":value})
    );
    let request = GoogleCommentWrite {
        content: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"content":null})
    );
    let value = serde_json::json!("value");
    let request = GoogleCommentWrite {
        anchor: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"anchor":value})
    );
    let request = GoogleCommentWrite {
        anchor: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"anchor":null})
    );
}

#[test]
fn googlereplywrite_partial_write_fields() {
    let value = serde_json::json!("value");
    let request = GoogleReplyWrite {
        content: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"content":value})
    );
    let request = GoogleReplyWrite {
        content: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"content":null})
    );
    let value = serde_json::json!("value");
    let request = GoogleReplyWrite {
        action: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"action":value})
    );
    let request = GoogleReplyWrite {
        action: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"action":null})
    );
}

#[test]
fn googlerevisionwrite_partial_write_fields() {
    let value = serde_json::json!(false);
    let request = GoogleRevisionWrite {
        keep_forever: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"keepForever":value})
    );
    let request = GoogleRevisionWrite {
        keep_forever: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"keepForever":null})
    );
    let value = serde_json::json!(false);
    let request = GoogleRevisionWrite {
        published: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"published":value})
    );
    let request = GoogleRevisionWrite {
        published: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"published":null})
    );
    let value = serde_json::json!(false);
    let request = GoogleRevisionWrite {
        publish_auto: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"publishAuto":value})
    );
    let request = GoogleRevisionWrite {
        publish_auto: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"publishAuto":null})
    );
    let value = serde_json::json!(false);
    let request = GoogleRevisionWrite {
        published_outside_domain: GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"publishedOutsideDomain":value})
    );
    let request = GoogleRevisionWrite {
        published_outside_domain: GooglePatch::Clear,
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"publishedOutsideDomain":null})
    );
}

#[test]
fn googleaccessproposallist_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"accessProposals":[{"fileId":"value","proposalId":"value","requesterEmailAddress":"value","recipientEmailAddress":"value","rolesAndViews":[{"role":"value","view":"value"}],"requestMessage":"value","createTime":"value"}],"nextPageToken":"value"}"#).unwrap();
    let value: GoogleAccessProposalList = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("accessProposals");
    let value: GoogleAccessProposalList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("accessProposals")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Array(vec![])
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("nextPageToken");
    let value: GoogleAccessProposalList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("nextPageToken")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleapplist_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"items":[{"id":"value","name":"value","objectType":"value","productUrl":"value","createUrl":"value","openUrlTemplate":"value","supportsCreate":false,"supportsImport":false,"installed":false,"authorized":false,"useByDefault":false,"primaryMimeTypes":["value"],"secondaryMimeTypes":["value"]}],"nextPageToken":"value"}"#).unwrap();
    let value: GoogleAppList = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("items");
    let value: GoogleAppList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("items")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Array(vec![])
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("nextPageToken");
    let value: GoogleAppList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("nextPageToken")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleapprovallist_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"items":[{"approvalId":"value","targetFileId":"value","createTime":"value","modifyTime":"value","completeTime":"value","dueTime":"value","status":"value","initiator":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"reviewerResponses":[{"reviewer":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"response":"value"}],"fileContentChangeBehavior":"value"}],"nextPageToken":"value"}"#).unwrap();
    let value: GoogleApprovalList = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("items");
    let value: GoogleApprovalList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("items")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Array(vec![])
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("nextPageToken");
    let value: GoogleApprovalList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("nextPageToken")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlecommentlist_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"comments":[{"id":"value","content":"value","createdTime":"value","modifiedTime":"value","author":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"resolved":false,"deleted":false,"replies":[{"id":"value","content":"value","createdTime":"value","modifiedTime":"value","author":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"action":"value","deleted":false}],"anchor":"value","quotedFileContent":{"mimeType":"value","value":"value"}}],"nextPageToken":"value"}"#).unwrap();
    let value: GoogleCommentList = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("comments");
    let value: GoogleCommentList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("comments")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Array(vec![])
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("nextPageToken");
    let value: GoogleCommentList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("nextPageToken")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googledrivelist_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"drives":[{"id":"value","name":"value","hidden":false,"createdTime":"value","restrictions":{"copyRequiresWriterPermission":false,"domainUsersOnly":false,"driveMembersOnly":false,"adminManagedRestrictions":false,"sharingFoldersRequiresOrganizerPermission":false},"capabilities":{"canAddChildren":false,"canComment":false,"canCopy":false,"canDeleteDrive":false,"canDownload":false,"canEdit":false,"canListChildren":false,"canManageMembers":false,"canReadRevisions":false,"canRename":false,"canShare":false},"colorRgb":"value","backgroundImageLink":"value","orgUnitId":"value"}],"nextPageToken":"value"}"#).unwrap();
    let value: GoogleDriveList = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("drives");
    let value: GoogleDriveList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("drives")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Array(vec![])
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("nextPageToken");
    let value: GoogleDriveList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("nextPageToken")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlelabellist_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"labels":[{"id":"value","revisionId":"value","fields":{"property":{"valueType":"value","text":["value"],"integer":["value"],"dateString":["value"],"selection":["value"],"user":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}]}}}],"nextPageToken":"value"}"#).unwrap();
    let value: GoogleLabelList = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("labels");
    let value: GoogleLabelList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("labels")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Array(vec![])
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("nextPageToken");
    let value: GoogleLabelList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("nextPageToken")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlepermissionlist_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"permissions":[{"id":"value","type":"value","role":"value","emailAddress":"value","domain":"value","allowFileDiscovery":false,"expirationTime":"value","displayName":"value","photoLink":"value","deleted":false,"pendingOwner":false,"permissionDetails":[{"permissionType":"value","role":"value","inherited":false,"inheritedFrom":"value"}],"view":"value"}],"nextPageToken":"value"}"#).unwrap();
    let value: GooglePermissionList = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("permissions");
    let value: GooglePermissionList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("permissions")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Array(vec![])
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("nextPageToken");
    let value: GooglePermissionList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("nextPageToken")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlereplylist_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"replies":[{"id":"value","content":"value","createdTime":"value","modifiedTime":"value","author":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"action":"value","deleted":false}],"nextPageToken":"value"}"#).unwrap();
    let value: GoogleReplyList = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("replies");
    let value: GoogleReplyList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("replies")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Array(vec![])
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("nextPageToken");
    let value: GoogleReplyList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("nextPageToken")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlerevisionlist_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"revisions":[{"id":"value","mimeType":"value","modifiedTime":"value","keepForever":false,"size":"value","md5Checksum":"value","published":false,"publishAuto":false,"publishedOutsideDomain":false,"originalFilename":"value","lastModifyingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"exportLinks":{"property":"value"}}],"nextPageToken":"value"}"#).unwrap();
    let value: GoogleRevisionList = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("revisions");
    let value: GoogleRevisionList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("revisions")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Array(vec![])
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("nextPageToken");
    let value: GoogleRevisionList = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("nextPageToken")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn aboutoptions_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value = serde_json::from_str(r#"{"fields":"value"}"#).unwrap();
    let value: AboutOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: AboutOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn applistoptions_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"appFilterExtensions":["value"],"appFilterMimeTypes":["value"],"languageCode":"value","fields":"value"}"#).unwrap();
    let value: AppListOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("appFilterExtensions");
    let value: AppListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("appFilterExtensions")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("appFilterMimeTypes");
    let value: AppListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("appFilterMimeTypes")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("languageCode");
    let value: AppListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("languageCode")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: AppListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn changescopeoptions_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"driveId":"value","supportsAllDrives":false}"#).unwrap();
    let value: ChangeScopeOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("driveId");
    let value: ChangeScopeOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("driveId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("supportsAllDrives");
    let value: ChangeScopeOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("supportsAllDrives")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn changelistoptions_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"pageToken":"value","driveId":"value","pageSize":2,"spaces":"value","includeRemoved":false,"includeCorpusRemovals":false,"includeItemsFromAllDrives":false,"supportsAllDrives":false,"fields":"value"}"#).unwrap();
    let value: ChangeListOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("pageToken");
    assert!(serde_json::from_value::<ChangeListOptions>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("driveId");
    let value: ChangeListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("driveId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("pageSize");
    let value: ChangeListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("pageSize")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("spaces");
    let value: ChangeListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("spaces")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("includeRemoved");
    let value: ChangeListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("includeRemoved")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("includeCorpusRemovals");
    let value: ChangeListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("includeCorpusRemovals")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("includeItemsFromAllDrives");
    let value: ChangeListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("includeItemsFromAllDrives")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("supportsAllDrives");
    let value: ChangeListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("supportsAllDrives")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: ChangeListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn changewatchoptions_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"pageToken":"value","driveId":"value","pageSize":2,"spaces":"value","includeRemoved":false,"includeItemsFromAllDrives":false,"supportsAllDrives":false,"fields":"value"}"#).unwrap();
    let value: ChangeWatchOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("pageToken");
    assert!(serde_json::from_value::<ChangeWatchOptions>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("driveId");
    let value: ChangeWatchOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("driveId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("pageSize");
    let value: ChangeWatchOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("pageSize")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("spaces");
    let value: ChangeWatchOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("spaces")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("includeRemoved");
    let value: ChangeWatchOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("includeRemoved")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("includeItemsFromAllDrives");
    let value: ChangeWatchOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("includeItemsFromAllDrives")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("supportsAllDrives");
    let value: ChangeWatchOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("supportsAllDrives")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: ChangeWatchOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn commentoptions_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"fields":"value","includeDeleted":false}"#).unwrap();
    let value: CommentOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: CommentOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("includeDeleted");
    let value: CommentOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("includeDeleted")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn commentlistoptions_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"fields":"value","pageSize":2,"pageToken":"value","includeDeleted":false,"startModifiedTime":"value"}"#).unwrap();
    let value: CommentListOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: CommentListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("pageSize");
    let value: CommentListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("pageSize")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("pageToken");
    let value: CommentListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("pageToken")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("includeDeleted");
    let value: CommentListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("includeDeleted")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("startModifiedTime");
    let value: CommentListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("startModifiedTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn csetokenoptions_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"fileId":"value","parent":"value"}"#).unwrap();
    let value: CseTokenOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fileId");
    let value: CseTokenOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fileId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("parent");
    let value: CseTokenOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("parent")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn drivedeleteoptions_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"allowItemDeletion":false,"useDomainAdminAccess":false}"#)
            .unwrap();
    let value: DriveDeleteOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("allowItemDeletion");
    let value: DriveDeleteOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("allowItemDeletion")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("useDomainAdminAccess");
    let value: DriveDeleteOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("useDomainAdminAccess")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn drivegetoptions_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"useDomainAdminAccess":false,"fields":"value"}"#).unwrap();
    let value: DriveGetOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("useDomainAdminAccess");
    let value: DriveGetOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("useDomainAdminAccess")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: DriveGetOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn drivelistoptions_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"pageSize":2,"pageToken":"value","q":"value","useDomainAdminAccess":false,"fields":"value"}"#).unwrap();
    let value: DriveListOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("pageSize");
    let value: DriveListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("pageSize")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("pageToken");
    let value: DriveListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("pageToken")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("q");
    let value: DriveListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("q")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("useDomainAdminAccess");
    let value: DriveListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("useDomainAdminAccess")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: DriveListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn driveupdateoptions_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"useDomainAdminAccess":false,"fields":"value"}"#).unwrap();
    let value: DriveUpdateOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("useDomainAdminAccess");
    let value: DriveUpdateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("useDomainAdminAccess")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: DriveUpdateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn emptytrashoptions_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value = serde_json::from_str(r#"{"driveId":"value"}"#).unwrap();
    let value: EmptyTrashOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("driveId");
    let value: EmptyTrashOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("driveId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn filecopyoptions_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value = serde_json::from_str(
        r#"{"supportsAllDrives":false,"ignoreDefaultVisibility":false,"fields":"value"}"#,
    )
    .unwrap();
    let value: FileCopyOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("supportsAllDrives");
    let value: FileCopyOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("supportsAllDrives")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("ignoreDefaultVisibility");
    let value: FileCopyOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("ignoreDefaultVisibility")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: FileCopyOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn filecreateoptions_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"supportsAllDrives":false,"ignoreDefaultVisibility":false,"useContentAsIndexableText":false,"keepRevisionForever":false,"fields":"value"}"#).unwrap();
    let value: FileCreateOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("supportsAllDrives");
    let value: FileCreateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("supportsAllDrives")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("ignoreDefaultVisibility");
    let value: FileCreateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("ignoreDefaultVisibility")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("useContentAsIndexableText");
    let value: FileCreateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("useContentAsIndexableText")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("keepRevisionForever");
    let value: FileCreateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("keepRevisionForever")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: FileCreateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn filedeleteoptions_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"supportsAllDrives":false,"enforceSingleParent":false}"#).unwrap();
    let value: FileDeleteOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("supportsAllDrives");
    let value: FileDeleteOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("supportsAllDrives")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("enforceSingleParent");
    let value: FileDeleteOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("enforceSingleParent")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn filedownloadoptions_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"mimeType":"value","revisionId":"value"}"#).unwrap();
    let value: FileDownloadOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("mimeType");
    let value: FileDownloadOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("mimeType")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("revisionId");
    let value: FileDownloadOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("revisionId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn filegetoptions_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"fields":"value","supportsAllDrives":false,"includeLabels":"value","includePermissionsForView":"value"}"#).unwrap();
    let value: FileGetOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: FileGetOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("supportsAllDrives");
    let value: FileGetOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("supportsAllDrives")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("includeLabels");
    let value: FileGetOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("includeLabels")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("includePermissionsForView");
    let value: FileGetOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("includePermissionsForView")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn filelistoptions_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"q":"value","corpora":"value","driveId":"value","pageSize":2,"pageToken":"value","spaces":"value","orderBy":"value","fields":"value","includeItemsFromAllDrives":false,"supportsAllDrives":false,"includePermissionsForView":"value"}"#).unwrap();
    let value: FileListOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("q");
    let value: FileListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("q")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("corpora");
    let value: FileListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("corpora")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("driveId");
    let value: FileListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("driveId")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("pageSize");
    let value: FileListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("pageSize")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("pageToken");
    let value: FileListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("pageToken")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("spaces");
    let value: FileListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("spaces")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("orderBy");
    let value: FileListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("orderBy")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: FileListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("includeItemsFromAllDrives");
    let value: FileListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("includeItemsFromAllDrives")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("supportsAllDrives");
    let value: FileListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("supportsAllDrives")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("includePermissionsForView");
    let value: FileListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("includePermissionsForView")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn fileupdateoptions_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"addParents":["value"],"removeParents":["value"],"supportsAllDrives":false,"keepRevisionForever":false,"useContentAsIndexableText":false,"fields":"value"}"#).unwrap();
    let value: FileUpdateOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("addParents");
    let value: FileUpdateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("addParents")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("removeParents");
    let value: FileUpdateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("removeParents")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("supportsAllDrives");
    let value: FileUpdateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("supportsAllDrives")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("keepRevisionForever");
    let value: FileUpdateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("keepRevisionForever")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("useContentAsIndexableText");
    let value: FileUpdateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("useContentAsIndexableText")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: FileUpdateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn filewatchoptions_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"acknowledgeAbuse":false,"supportsAllDrives":false}"#).unwrap();
    let value: FileWatchOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("acknowledgeAbuse");
    let value: FileWatchOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("acknowledgeAbuse")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("supportsAllDrives");
    let value: FileWatchOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("supportsAllDrives")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn generateidsoptions_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"count":2,"space":"value","type":"value"}"#).unwrap();
    let value: GenerateIdsOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("count");
    let value: GenerateIdsOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("count")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("space");
    let value: GenerateIdsOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("space")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("type");
    let value: GenerateIdsOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("type")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn labellistoptions_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"pageSize":2,"pageToken":"value"}"#).unwrap();
    let value: LabelListOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("pageSize");
    let value: LabelListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("pageSize")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("pageToken");
    let value: LabelListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("pageToken")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn permissioncreateoptions_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"sendNotificationEmail":false,"emailMessage":"value","supportsAllDrives":false,"transferOwnership":false,"useDomainAdminAccess":false,"fields":"value"}"#).unwrap();
    let value: PermissionCreateOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("sendNotificationEmail");
    let value: PermissionCreateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("sendNotificationEmail")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("emailMessage");
    let value: PermissionCreateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("emailMessage")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("supportsAllDrives");
    let value: PermissionCreateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("supportsAllDrives")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("transferOwnership");
    let value: PermissionCreateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("transferOwnership")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("useDomainAdminAccess");
    let value: PermissionCreateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("useDomainAdminAccess")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: PermissionCreateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn permissiondeleteoptions_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"supportsAllDrives":false,"useDomainAdminAccess":false}"#)
            .unwrap();
    let value: PermissionDeleteOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("supportsAllDrives");
    let value: PermissionDeleteOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("supportsAllDrives")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("useDomainAdminAccess");
    let value: PermissionDeleteOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("useDomainAdminAccess")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn permissiongetoptions_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value = serde_json::from_str(
        r#"{"supportsAllDrives":false,"useDomainAdminAccess":false,"fields":"value"}"#,
    )
    .unwrap();
    let value: PermissionGetOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("supportsAllDrives");
    let value: PermissionGetOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("supportsAllDrives")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("useDomainAdminAccess");
    let value: PermissionGetOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("useDomainAdminAccess")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: PermissionGetOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn permissionlistoptions_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"supportsAllDrives":false,"useDomainAdminAccess":false,"pageSize":2,"pageToken":"value","fields":"value"}"#).unwrap();
    let value: PermissionListOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("supportsAllDrives");
    let value: PermissionListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("supportsAllDrives")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("useDomainAdminAccess");
    let value: PermissionListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("useDomainAdminAccess")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("pageSize");
    let value: PermissionListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("pageSize")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("pageToken");
    let value: PermissionListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("pageToken")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: PermissionListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn permissionupdateoptions_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"supportsAllDrives":false,"useDomainAdminAccess":false,"removeExpiration":false,"transferOwnership":false,"fields":"value"}"#).unwrap();
    let value: PermissionUpdateOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("supportsAllDrives");
    let value: PermissionUpdateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("supportsAllDrives")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("useDomainAdminAccess");
    let value: PermissionUpdateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("useDomainAdminAccess")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("removeExpiration");
    let value: PermissionUpdateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("removeExpiration")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("transferOwnership");
    let value: PermissionUpdateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("transferOwnership")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: PermissionUpdateOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn replyoptions_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"fields":"value","includeDeleted":false}"#).unwrap();
    let value: ReplyOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: ReplyOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("includeDeleted");
    let value: ReplyOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("includeDeleted")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn replylistoptions_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value = serde_json::from_str(
        r#"{"fields":"value","pageSize":2,"pageToken":"value","includeDeleted":false}"#,
    )
    .unwrap();
    let value: ReplyListOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: ReplyListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("pageSize");
    let value: ReplyListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("pageSize")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("pageToken");
    let value: ReplyListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("pageToken")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("includeDeleted");
    let value: ReplyListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("includeDeleted")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn revisiongetoptions_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"fields":"value","acknowledgeAbuse":false}"#).unwrap();
    let value: RevisionGetOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: RevisionGetOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("acknowledgeAbuse");
    let value: RevisionGetOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("acknowledgeAbuse")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn revisionlistoptions_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"fields":"value","pageSize":2,"pageToken":"value"}"#).unwrap();
    let value: RevisionListOptions = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fields");
    let value: RevisionListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fields")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("pageSize");
    let value: RevisionListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("pageSize")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("pageToken");
    let value: RevisionListOptions = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("pageToken")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlechannelrequest_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"id":"value","type":"value","address":"value","token":"value","expiration":"value","params":{"property":"value"}}"#).unwrap();
    let value: GoogleChannelRequest = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("id");
    assert!(serde_json::from_value::<GoogleChannelRequest>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("type");
    assert!(serde_json::from_value::<GoogleChannelRequest>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("address");
    assert!(serde_json::from_value::<GoogleChannelRequest>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("token");
    let value: GoogleChannelRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("token")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("expiration");
    let value: GoogleChannelRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("expiration")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("params");
    let value: GoogleChannelRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("params")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlechannelstoprequest_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"id":"value","resourceId":"value"}"#).unwrap();
    let value: GoogleChannelStopRequest = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("id");
    assert!(serde_json::from_value::<GoogleChannelStopRequest>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("resourceId");
    assert!(serde_json::from_value::<GoogleChannelStopRequest>(omitted).is_err());
}

#[test]
fn googlemodifylabelsrequest_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"labelModifications":[{"labelId":"value","fieldModifications":[{"fieldId":"value","setTextValues":["value"],"setIntegerValues":["value"],"setDateValues":["value"],"setSelectionValues":["value"],"unsetValues":false}],"removeLabel":false}]}"#).unwrap();
    let value: GoogleModifyLabelsRequest = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("labelModifications");
    let value: GoogleModifyLabelsRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("labelModifications")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Array(vec![])
    );
}

#[test]
fn googlelabelmodification_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"labelId":"value","fieldModifications":[{"fieldId":"value","setTextValues":["value"],"setIntegerValues":["value"],"setDateValues":["value"],"setSelectionValues":["value"],"unsetValues":false}],"removeLabel":false}"#).unwrap();
    let value: GoogleLabelModification = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("labelId");
    assert!(serde_json::from_value::<GoogleLabelModification>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("fieldModifications");
    let value: GoogleLabelModification = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fieldModifications")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Array(vec![])
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("removeLabel");
    let value: GoogleLabelModification = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("removeLabel")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlelabelfieldmodification_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"fieldId":"value","setTextValues":["value"],"setIntegerValues":["value"],"setDateValues":["value"],"setSelectionValues":["value"],"unsetValues":false}"#).unwrap();
    let value: GoogleLabelFieldModification = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("fieldId");
    assert!(serde_json::from_value::<GoogleLabelFieldModification>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("setTextValues");
    let value: GoogleLabelFieldModification = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("setTextValues")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("setIntegerValues");
    let value: GoogleLabelFieldModification = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("setIntegerValues")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("setDateValues");
    let value: GoogleLabelFieldModification = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("setDateValues")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("setSelectionValues");
    let value: GoogleLabelFieldModification = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("setSelectionValues")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("unsetValues");
    let value: GoogleLabelFieldModification = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("unsetValues")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn approveapprovalrequest_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value = serde_json::from_str(r#"{"message":"value"}"#).unwrap();
    let value: ApproveApprovalRequest = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("message");
    let value: ApproveApprovalRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("message")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn cancelapprovalrequest_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value = serde_json::from_str(r#"{"message":"value"}"#).unwrap();
    let value: CancelApprovalRequest = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("message");
    let value: CancelApprovalRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("message")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn commentapprovalrequest_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value = serde_json::from_str(r#"{"message":"value"}"#).unwrap();
    let value: CommentApprovalRequest = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("message");
    assert!(serde_json::from_value::<CommentApprovalRequest>(omitted).is_err());
}

#[test]
fn declineapprovalrequest_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value = serde_json::from_str(r#"{"message":"value"}"#).unwrap();
    let value: DeclineApprovalRequest = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("message");
    let value: DeclineApprovalRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("message")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn reassignapprovalrequest_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"replaceReviewers":[{"removedReviewerEmail":"value","addedReviewerEmail":"value"}],"addReviewers":[{"addedReviewerEmail":"value"}],"message":"value"}"#).unwrap();
    let value: ReassignApprovalRequest = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("replaceReviewers");
    let value: ReassignApprovalRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("replaceReviewers")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("addReviewers");
    let value: ReassignApprovalRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("addReviewers")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("message");
    let value: ReassignApprovalRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("message")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlereplacereviewer_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"removedReviewerEmail":"value","addedReviewerEmail":"value"}"#)
            .unwrap();
    let value: GoogleReplaceReviewer = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("removedReviewerEmail");
    assert!(serde_json::from_value::<GoogleReplaceReviewer>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("addedReviewerEmail");
    assert!(serde_json::from_value::<GoogleReplaceReviewer>(omitted).is_err());
}

#[test]
fn googleaddreviewer_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value =
        serde_json::from_str(r#"{"addedReviewerEmail":"value"}"#).unwrap();
    let value: GoogleAddReviewer = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("addedReviewerEmail");
    assert!(serde_json::from_value::<GoogleAddReviewer>(omitted).is_err());
}

#[test]
fn startapprovalrequest_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"reviewerEmails":["value"],"lockFile":false,"message":"value","dueTime":"value","fileContentChangeBehavior":"value"}"#).unwrap();
    let value: StartApprovalRequest = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("reviewerEmails");
    let value: StartApprovalRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("reviewerEmails")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Array(vec![])
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("lockFile");
    let value: StartApprovalRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("lockFile")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("message");
    let value: StartApprovalRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("message")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("dueTime");
    let value: StartApprovalRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("dueTime")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("fileContentChangeBehavior");
    let value: StartApprovalRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("fileContentChangeBehavior")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleapprovalreviewer_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value = serde_json::from_str(r#"{"emailAddress":"value"}"#).unwrap();
    let value: GoogleApprovalReviewer = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("emailAddress");
    assert!(serde_json::from_value::<GoogleApprovalReviewer>(omitted).is_err());
}

#[test]
fn resolveaccessproposalrequest_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value = serde_json::from_str(
        r#"{"action":"value","role":["value"],"view":"value","sendNotification":false}"#,
    )
    .unwrap();
    let value: ResolveAccessProposalRequest = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("action");
    assert!(serde_json::from_value::<ResolveAccessProposalRequest>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("role");
    let value: ResolveAccessProposalRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("role")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("view");
    let value: ResolveAccessProposalRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("view")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("sendNotification");
    let value: ResolveAccessProposalRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("sendNotification")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleconfigsources_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"client_id_env":"value","client_secret_env":"value","oauth_file_env":"value","bundled_config_path":"value"}"#).unwrap();
    let value: GoogleConfigSources = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("client_id_env");
    let value: GoogleConfigSources = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("client_id_env")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("client_secret_env");
    let value: GoogleConfigSources = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("client_secret_env")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("oauth_file_env");
    let value: GoogleConfigSources = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("oauth_file_env")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("bundled_config_path");
    let value: GoogleConfigSources = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("bundled_config_path")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleoauthconfig_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"client_id":"value","client_secret":"value","redirect_uris":["value"],"scopes":["value"]}"#).unwrap();
    let value: GoogleOAuthConfig = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("client_id");
    assert!(serde_json::from_value::<GoogleOAuthConfig>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("client_secret");
    let value: GoogleOAuthConfig = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("client_secret")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("redirect_uris");
    let value: GoogleOAuthConfig = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("redirect_uris")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Array(vec![])
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("scopes");
    let value: GoogleOAuthConfig = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("scopes")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Array(vec![])
    );
}

#[test]
fn googleauthorizationrequest_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"redirect_uri":"value","state":"value","code_challenge":"value","code_challenge_method":"value","scopes":["value"],"access_type":"value","prompt":"value","login_hint":"value"}"#).unwrap();
    let value: GoogleAuthorizationRequest = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("redirect_uri");
    assert!(serde_json::from_value::<GoogleAuthorizationRequest>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("state");
    assert!(serde_json::from_value::<GoogleAuthorizationRequest>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("code_challenge");
    assert!(serde_json::from_value::<GoogleAuthorizationRequest>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("code_challenge_method");
    assert!(serde_json::from_value::<GoogleAuthorizationRequest>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("scopes");
    let value: GoogleAuthorizationRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("scopes")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Array(vec![])
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("access_type");
    let value: GoogleAuthorizationRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("access_type")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("prompt");
    let value: GoogleAuthorizationRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("prompt")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("login_hint");
    let value: GoogleAuthorizationRequest = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("login_hint")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleauthorizationurl_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value = serde_json::from_str(r#"{"url":"value"}"#).unwrap();
    let value: GoogleAuthorizationUrl = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("url");
    assert!(serde_json::from_value::<GoogleAuthorizationUrl>(omitted).is_err());
}

#[test]
fn googletokenresponse_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"access_token":"value","token_type":"value","expires_in":2,"refresh_token":"value","scope":"value","id_token":"value","refresh_token_expires_in":2}"#).unwrap();
    let value: GoogleTokenResponse = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("access_token");
    assert!(serde_json::from_value::<GoogleTokenResponse>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("token_type");
    assert!(serde_json::from_value::<GoogleTokenResponse>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("expires_in");
    assert!(serde_json::from_value::<GoogleTokenResponse>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("refresh_token");
    let value: GoogleTokenResponse = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("refresh_token")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("scope");
    let value: GoogleTokenResponse = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("scope")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("id_token");
    let value: GoogleTokenResponse = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("id_token")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("refresh_token_expires_in");
    let value: GoogleTokenResponse = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("refresh_token_expires_in")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleuserinfo_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"sub":"value","email":"value","email_verified":false,"name":"value","picture":"value","hd":"value"}"#).unwrap();
    let value: GoogleUserInfo = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("sub");
    assert!(serde_json::from_value::<GoogleUserInfo>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("email");
    let value: GoogleUserInfo = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("email")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("email_verified");
    let value: GoogleUserInfo = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("email_verified")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("name");
    let value: GoogleUserInfo = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("name")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("picture");
    let value: GoogleUserInfo = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("picture")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("hd");
    let value: GoogleUserInfo = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("hd")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleidtokenvalidation_fields_roundtrip_and_preserve_omission() {
    let expected: serde_json::Value = serde_json::from_str(
        r#"{"issuer":"value","audience":"value","nonce":"value","clock_skew_seconds":2}"#,
    )
    .unwrap();
    let value: GoogleIdTokenValidation = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("issuer");
    assert!(serde_json::from_value::<GoogleIdTokenValidation>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("audience");
    assert!(serde_json::from_value::<GoogleIdTokenValidation>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("nonce");
    let value: GoogleIdTokenValidation = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("nonce")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("clock_skew_seconds");
    let value: GoogleIdTokenValidation = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("clock_skew_seconds")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleidclaims_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"iss":"value","aud":"client-id","azp":"value","sub":"value","exp":2,"iat":2,"email":"value","email_verified":false,"nonce":"value"}"#).unwrap();
    let value: GoogleIdClaims = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("iss");
    assert!(serde_json::from_value::<GoogleIdClaims>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("aud");
    assert!(serde_json::from_value::<GoogleIdClaims>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("azp");
    let value: GoogleIdClaims = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("azp")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("sub");
    assert!(serde_json::from_value::<GoogleIdClaims>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("exp");
    assert!(serde_json::from_value::<GoogleIdClaims>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("iat");
    assert!(serde_json::from_value::<GoogleIdClaims>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("email");
    let value: GoogleIdClaims = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("email")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("email_verified");
    let value: GoogleIdClaims = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("email_verified")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("nonce");
    let value: GoogleIdClaims = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("nonce")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googleoidcdiscovery_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"issuer":"value","authorization_endpoint":"value","token_endpoint":"value","userinfo_endpoint":"value","jwks_uri":"value","revocation_endpoint":"value","scopes_supported":["value"]}"#).unwrap();
    let value: GoogleOidcDiscovery = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("issuer");
    assert!(serde_json::from_value::<GoogleOidcDiscovery>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("authorization_endpoint");
    assert!(serde_json::from_value::<GoogleOidcDiscovery>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("token_endpoint");
    assert!(serde_json::from_value::<GoogleOidcDiscovery>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("userinfo_endpoint");
    assert!(serde_json::from_value::<GoogleOidcDiscovery>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("jwks_uri");
    assert!(serde_json::from_value::<GoogleOidcDiscovery>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted
        .as_object_mut()
        .unwrap()
        .remove("revocation_endpoint");
    let value: GoogleOidcDiscovery = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("revocation_endpoint")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("scopes_supported");
    let value: GoogleOidcDiscovery = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("scopes_supported")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}

#[test]
fn googlejwkset_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"keys":[{"kid":"value","kty":"value","alg":"value","use":"value","n":"value","e":"value","crv":"value","x":"value","y":"value"}]}"#).unwrap();
    let value: GoogleJwkSet = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("keys");
    let value: GoogleJwkSet = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("keys")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Array(vec![])
    );
}

#[test]
fn googlejwk_fields_roundtrip_and_preserve_omission() {
    let expected:serde_json::Value=serde_json::from_str(r#"{"kid":"value","kty":"value","alg":"value","use":"value","n":"value","e":"value","crv":"value","x":"value","y":"value"}"#).unwrap();
    let value: GoogleJwk = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("kid");
    let value: GoogleJwk = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("kid")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("kty");
    assert!(serde_json::from_value::<GoogleJwk>(omitted).is_err());
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("alg");
    let value: GoogleJwk = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("alg")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("use");
    let value: GoogleJwk = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("use")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("n");
    let value: GoogleJwk = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("n")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("e");
    let value: GoogleJwk = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("e")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("crv");
    let value: GoogleJwk = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("crv")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("x");
    let value: GoogleJwk = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("x")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
    let mut omitted = expected.clone();
    omitted.as_object_mut().unwrap().remove("y");
    let value: GoogleJwk = serde_json::from_value(omitted).unwrap();
    assert_eq!(
        serde_json::to_value(value)
            .unwrap()
            .get("y")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        serde_json::Value::Null
    );
}
