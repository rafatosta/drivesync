//! Golden projections fixed from the public contract, independent of HTTP builders.
use crate::providers::google::*;

fn projection<T: serde::de::DeserializeOwned + serde::Serialize>(
    raw: &str,
    optional: &[&str],
    lists: &[&str],
    required: &[&str],
) {
    let expected: serde_json::Value = serde_json::from_str(raw).unwrap();
    let value: T = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(value).unwrap(), expected);
    for key in optional {
        let mut omitted = expected.clone();
        omitted.as_object_mut().unwrap().remove(*key);
        let value: T = serde_json::from_value(omitted).unwrap();
        assert_eq!(
            serde_json::to_value(value)
                .unwrap()
                .get(*key)
                .cloned()
                .unwrap_or(serde_json::Value::Null),
            serde_json::Value::Null,
            "{key}"
        );
    }
    for key in lists {
        let mut omitted = expected.clone();
        omitted.as_object_mut().unwrap().remove(*key);
        let value: T = serde_json::from_value(omitted).unwrap();
        assert_eq!(
            serde_json::to_value(value)
                .unwrap()
                .get(*key)
                .cloned()
                .unwrap_or(serde_json::Value::Null),
            serde_json::Value::Array(vec![]),
            "{key}"
        );
    }
    for key in required {
        let mut omitted = expected.clone();
        omitted.as_object_mut().unwrap().remove(*key);
        assert!(serde_json::from_value::<T>(omitted).is_err(), "{key}");
    }
}

macro_rules! patch_field {
    ($model:ident,$field:ident,$key:literal,$fixture:tt) => {{
        let value=serde_json::json!($fixture);
        let request=$model{$field:GooglePatch::Set(serde_json::from_value(value.clone()).unwrap()),..Default::default()};
        assert_eq!(serde_json::to_value(request).unwrap(),serde_json::json!({$key:value}));
        let request=$model{$field:GooglePatch::Clear,..Default::default()};
        assert_eq!(serde_json::to_value(request).unwrap(),serde_json::json!({$key:null}));
    }};
}

#[test]
fn googleabout_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleAbout>(
        r#"{"user":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"storageQuota":{"limit":"value","usage":"value","usageInDrive":"value","usageInDriveTrash":"value"},"importFormats":{"property":["value"]},"exportFormats":{"property":["value"]},"maxImportSizes":{"property":"value"},"appInstalled":false,"folderColorPalette":["value"]}"#,
        &[
            "user",
            "storageQuota",
            "importFormats",
            "exportFormats",
            "maxImportSizes",
            "appInstalled",
            "folderColorPalette",
        ],
        &[],
        &[],
    );
}

#[test]
fn googleuser_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleUser>(
        r#"{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}"#,
        &[
            "displayName",
            "emailAddress",
            "photoLink",
            "permissionId",
            "me",
        ],
        &[],
        &[],
    );
}

#[test]
fn googlestoragequota_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleStorageQuota>(
        r#"{"limit":"value","usage":"value","usageInDrive":"value","usageInDriveTrash":"value"}"#,
        &["limit", "usage", "usageInDrive", "usageInDriveTrash"],
        &[],
        &[],
    );
}

#[test]
fn googlefile_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleFile>(
        r#"{"id":"value","name":"value","mimeType":"value","parents":["value"],"description":"value","trashed":false,"explicitlyTrashed":false,"trashedTime":"value","version":"value","createdTime":"value","modifiedTime":"value","modifiedByMeTime":"value","viewedByMeTime":"value","size":"value","quotaBytesUsed":"value","md5Checksum":"value","sha1Checksum":"value","sha256Checksum":"value","headRevisionId":"value","driveId":"value","resourceKey":"value","webViewLink":"value","webContentLink":"value","thumbnailLink":"value","iconLink":"value","fileExtension":"value","fullFileExtension":"value","originalFilename":"value","shortcutDetails":{"targetId":"value","targetMimeType":"value","targetResourceKey":"value"},"capabilities":{"canEdit":false,"canCopy":false,"canDownload":false,"canDelete":false,"canRename":false,"canMoveItemWithinDrive":false,"canMoveItemOutOfDrive":false,"canAddChildren":false,"canListChildren":false,"canShare":false,"canComment":false,"canReadRevisions":false,"canChangeCopyRequiresWriterPermission":false},"permissions":[{"id":"value","type":"value","role":"value","emailAddress":"value","domain":"value","allowFileDiscovery":false,"expirationTime":"value","displayName":"value","photoLink":"value","deleted":false,"pendingOwner":false,"permissionDetails":[{"permissionType":"value","role":"value","inherited":false,"inheritedFrom":"value"}],"view":"value"}],"owners":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}],"lastModifyingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"sharingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"shared":false,"ownedByMe":false,"modifiedByMe":false,"starred":false,"folderColorRgb":"value","properties":{"property":"value"},"appProperties":{"property":"value"},"contentHints":{"indexableText":"value","thumbnail":{"image":"value","mimeType":"value"}},"imageMediaMetadata":{"width":2,"height":2,"rotation":2,"time":"value","cameraMake":"value","cameraModel":"value","exposureTime":1.5,"aperture":1.5,"isoSpeed":2,"location":{"latitude":1.5,"longitude":1.5,"altitude":1.5}},"videoMediaMetadata":{"width":2,"height":2,"durationMillis":"value"},"labelInfo":{"labels":[{"id":"value","revisionId":"value","fields":{"property":{"valueType":"value","text":["value"],"integer":["value"],"dateString":["value"],"selection":["value"],"user":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}]}}}]},"contentRestrictions":[{"readOnly":false,"reason":"value","restrictingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"restrictionTime":"value","ownerRestricted":false,"systemRestricted":false}],"downloadRestrictions":{"itemDownloadRestriction":{"restrictedForReaders":false,"restrictedForWriters":false},"effectiveDownloadRestrictionWithContext":{"restrictedForReaders":false,"restrictedForWriters":false}},"clientEncryptionDetails":{"encryptionState":"value","decryptionMetadata":{"wrappedKey":"value","kaclsId":"value"}},"copyRequiresWriterPermission":false,"writersCanShare":false,"hasThumbnail":false,"spaces":["value"],"permissionIds":["value"],"teamDriveId":"value","inheritedPermissionsDisabled":false}"#,
        &[
            "id",
            "name",
            "mimeType",
            "parents",
            "description",
            "trashed",
            "explicitlyTrashed",
            "trashedTime",
            "version",
            "createdTime",
            "modifiedTime",
            "modifiedByMeTime",
            "viewedByMeTime",
            "size",
            "quotaBytesUsed",
            "md5Checksum",
            "sha1Checksum",
            "sha256Checksum",
            "headRevisionId",
            "driveId",
            "resourceKey",
            "webViewLink",
            "webContentLink",
            "thumbnailLink",
            "iconLink",
            "fileExtension",
            "fullFileExtension",
            "originalFilename",
            "shortcutDetails",
            "capabilities",
            "permissions",
            "owners",
            "lastModifyingUser",
            "sharingUser",
            "shared",
            "ownedByMe",
            "modifiedByMe",
            "starred",
            "folderColorRgb",
            "properties",
            "appProperties",
            "contentHints",
            "imageMediaMetadata",
            "videoMediaMetadata",
            "labelInfo",
            "contentRestrictions",
            "downloadRestrictions",
            "clientEncryptionDetails",
            "copyRequiresWriterPermission",
            "writersCanShare",
            "hasThumbnail",
            "spaces",
            "permissionIds",
            "teamDriveId",
            "inheritedPermissionsDisabled",
        ],
        &[],
        &[],
    );
}

#[test]
fn googleshortcutdetails_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleShortcutDetails>(
        r#"{"targetId":"value","targetMimeType":"value","targetResourceKey":"value"}"#,
        &["targetId", "targetMimeType", "targetResourceKey"],
        &[],
        &[],
    );
}

#[test]
fn googlefilecapabilities_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleFileCapabilities>(
        r#"{"canEdit":false,"canCopy":false,"canDownload":false,"canDelete":false,"canRename":false,"canMoveItemWithinDrive":false,"canMoveItemOutOfDrive":false,"canAddChildren":false,"canListChildren":false,"canShare":false,"canComment":false,"canReadRevisions":false,"canChangeCopyRequiresWriterPermission":false}"#,
        &[
            "canEdit",
            "canCopy",
            "canDownload",
            "canDelete",
            "canRename",
            "canMoveItemWithinDrive",
            "canMoveItemOutOfDrive",
            "canAddChildren",
            "canListChildren",
            "canShare",
            "canComment",
            "canReadRevisions",
            "canChangeCopyRequiresWriterPermission",
        ],
        &[],
        &[],
    );
}

#[test]
fn googlecontenthints_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleContentHints>(
        r#"{"indexableText":"value","thumbnail":{"image":"value","mimeType":"value"}}"#,
        &["indexableText", "thumbnail"],
        &[],
        &[],
    );
}

#[test]
fn googlethumbnail_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleThumbnail>(
        r#"{"image":"value","mimeType":"value"}"#,
        &["image", "mimeType"],
        &[],
        &[],
    );
}

#[test]
fn googleimagemediametadata_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleImageMediaMetadata>(
        r#"{"width":2,"height":2,"rotation":2,"time":"value","cameraMake":"value","cameraModel":"value","exposureTime":1.5,"aperture":1.5,"isoSpeed":2,"location":{"latitude":1.5,"longitude":1.5,"altitude":1.5}}"#,
        &[
            "width",
            "height",
            "rotation",
            "time",
            "cameraMake",
            "cameraModel",
            "exposureTime",
            "aperture",
            "isoSpeed",
            "location",
        ],
        &[],
        &[],
    );
}

#[test]
fn googlegeolocation_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleGeoLocation>(
        r#"{"latitude":1.5,"longitude":1.5,"altitude":1.5}"#,
        &["latitude", "longitude", "altitude"],
        &[],
        &[],
    );
}

#[test]
fn googlevideomediametadata_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleVideoMediaMetadata>(
        r#"{"width":2,"height":2,"durationMillis":"value"}"#,
        &["width", "height", "durationMillis"],
        &[],
        &[],
    );
}

#[test]
fn googlecontentrestriction_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleContentRestriction>(
        r#"{"readOnly":false,"reason":"value","restrictingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"restrictionTime":"value","ownerRestricted":false,"systemRestricted":false}"#,
        &[
            "readOnly",
            "reason",
            "restrictingUser",
            "restrictionTime",
            "ownerRestricted",
            "systemRestricted",
        ],
        &[],
        &[],
    );
}

#[test]
fn googledownloadrestrictions_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleDownloadRestrictions>(
        r#"{"itemDownloadRestriction":{"restrictedForReaders":false,"restrictedForWriters":false},"effectiveDownloadRestrictionWithContext":{"restrictedForReaders":false,"restrictedForWriters":false}}"#,
        &[
            "itemDownloadRestriction",
            "effectiveDownloadRestrictionWithContext",
        ],
        &[],
        &[],
    );
}

#[test]
fn googledownloadrestriction_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleDownloadRestriction>(
        r#"{"restrictedForReaders":false,"restrictedForWriters":false}"#,
        &["restrictedForReaders", "restrictedForWriters"],
        &[],
        &[],
    );
}

#[test]
fn googleclientencryptiondetails_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleClientEncryptionDetails>(
        r#"{"encryptionState":"value","decryptionMetadata":{"wrappedKey":"value","kaclsId":"value"}}"#,
        &["encryptionState", "decryptionMetadata"],
        &[],
        &[],
    );
}

#[test]
fn googledecryptionmetadata_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleDecryptionMetadata>(
        r#"{"wrappedKey":"value","kaclsId":"value"}"#,
        &["wrappedKey", "kaclsId"],
        &[],
        &[],
    );
}

#[test]
fn googlelabelinfo_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleLabelInfo>(
        r#"{"labels":[{"id":"value","revisionId":"value","fields":{"property":{"valueType":"value","text":["value"],"integer":["value"],"dateString":["value"],"selection":["value"],"user":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}]}}}]}"#,
        &["labels"],
        &[],
        &[],
    );
}

#[test]
fn googlelabel_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleLabel>(
        r#"{"id":"value","revisionId":"value","fields":{"property":{"valueType":"value","text":["value"],"integer":["value"],"dateString":["value"],"selection":["value"],"user":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}]}}}"#,
        &["id", "revisionId", "fields"],
        &[],
        &[],
    );
}

#[test]
fn googlelabelfield_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleLabelField>(
        r#"{"valueType":"value","text":["value"],"integer":["value"],"dateString":["value"],"selection":["value"],"user":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}]}"#,
        &[
            "valueType",
            "text",
            "integer",
            "dateString",
            "selection",
            "user",
        ],
        &[],
        &[],
    );
}

#[test]
fn googlefilelist_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleFileList>(
        r#"{"files":[{"id":"value","name":"value","mimeType":"value","parents":["value"],"description":"value","trashed":false,"explicitlyTrashed":false,"trashedTime":"value","version":"value","createdTime":"value","modifiedTime":"value","modifiedByMeTime":"value","viewedByMeTime":"value","size":"value","quotaBytesUsed":"value","md5Checksum":"value","sha1Checksum":"value","sha256Checksum":"value","headRevisionId":"value","driveId":"value","resourceKey":"value","webViewLink":"value","webContentLink":"value","thumbnailLink":"value","iconLink":"value","fileExtension":"value","fullFileExtension":"value","originalFilename":"value","shortcutDetails":{"targetId":"value","targetMimeType":"value","targetResourceKey":"value"},"capabilities":{"canEdit":false,"canCopy":false,"canDownload":false,"canDelete":false,"canRename":false,"canMoveItemWithinDrive":false,"canMoveItemOutOfDrive":false,"canAddChildren":false,"canListChildren":false,"canShare":false,"canComment":false,"canReadRevisions":false,"canChangeCopyRequiresWriterPermission":false},"permissions":[{"id":"value","type":"value","role":"value","emailAddress":"value","domain":"value","allowFileDiscovery":false,"expirationTime":"value","displayName":"value","photoLink":"value","deleted":false,"pendingOwner":false,"permissionDetails":[{"permissionType":"value","role":"value","inherited":false,"inheritedFrom":"value"}],"view":"value"}],"owners":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}],"lastModifyingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"sharingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"shared":false,"ownedByMe":false,"modifiedByMe":false,"starred":false,"folderColorRgb":"value","properties":{"property":"value"},"appProperties":{"property":"value"},"contentHints":{"indexableText":"value","thumbnail":{"image":"value","mimeType":"value"}},"imageMediaMetadata":{"width":2,"height":2,"rotation":2,"time":"value","cameraMake":"value","cameraModel":"value","exposureTime":1.5,"aperture":1.5,"isoSpeed":2,"location":{"latitude":1.5,"longitude":1.5,"altitude":1.5}},"videoMediaMetadata":{"width":2,"height":2,"durationMillis":"value"},"labelInfo":{"labels":[{"id":"value","revisionId":"value","fields":{"property":{"valueType":"value","text":["value"],"integer":["value"],"dateString":["value"],"selection":["value"],"user":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}]}}}]},"contentRestrictions":[{"readOnly":false,"reason":"value","restrictingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"restrictionTime":"value","ownerRestricted":false,"systemRestricted":false}],"downloadRestrictions":{"itemDownloadRestriction":{"restrictedForReaders":false,"restrictedForWriters":false},"effectiveDownloadRestrictionWithContext":{"restrictedForReaders":false,"restrictedForWriters":false}},"clientEncryptionDetails":{"encryptionState":"value","decryptionMetadata":{"wrappedKey":"value","kaclsId":"value"}},"copyRequiresWriterPermission":false,"writersCanShare":false,"hasThumbnail":false,"spaces":["value"],"permissionIds":["value"],"teamDriveId":"value","inheritedPermissionsDisabled":false}],"nextPageToken":"value","incompleteSearch":false}"#,
        &["nextPageToken", "incompleteSearch"],
        &["files"],
        &[],
    );
}

#[test]
fn googlechange_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleChange>(
        r#"{"fileId":"value","file":{"id":"value","name":"value","mimeType":"value","parents":["value"],"description":"value","trashed":false,"explicitlyTrashed":false,"trashedTime":"value","version":"value","createdTime":"value","modifiedTime":"value","modifiedByMeTime":"value","viewedByMeTime":"value","size":"value","quotaBytesUsed":"value","md5Checksum":"value","sha1Checksum":"value","sha256Checksum":"value","headRevisionId":"value","driveId":"value","resourceKey":"value","webViewLink":"value","webContentLink":"value","thumbnailLink":"value","iconLink":"value","fileExtension":"value","fullFileExtension":"value","originalFilename":"value","shortcutDetails":{"targetId":"value","targetMimeType":"value","targetResourceKey":"value"},"capabilities":{"canEdit":false,"canCopy":false,"canDownload":false,"canDelete":false,"canRename":false,"canMoveItemWithinDrive":false,"canMoveItemOutOfDrive":false,"canAddChildren":false,"canListChildren":false,"canShare":false,"canComment":false,"canReadRevisions":false,"canChangeCopyRequiresWriterPermission":false},"permissions":[{"id":"value","type":"value","role":"value","emailAddress":"value","domain":"value","allowFileDiscovery":false,"expirationTime":"value","displayName":"value","photoLink":"value","deleted":false,"pendingOwner":false,"permissionDetails":[{"permissionType":"value","role":"value","inherited":false,"inheritedFrom":"value"}],"view":"value"}],"owners":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}],"lastModifyingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"sharingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"shared":false,"ownedByMe":false,"modifiedByMe":false,"starred":false,"folderColorRgb":"value","properties":{"property":"value"},"appProperties":{"property":"value"},"contentHints":{"indexableText":"value","thumbnail":{"image":"value","mimeType":"value"}},"imageMediaMetadata":{"width":2,"height":2,"rotation":2,"time":"value","cameraMake":"value","cameraModel":"value","exposureTime":1.5,"aperture":1.5,"isoSpeed":2,"location":{"latitude":1.5,"longitude":1.5,"altitude":1.5}},"videoMediaMetadata":{"width":2,"height":2,"durationMillis":"value"},"labelInfo":{"labels":[{"id":"value","revisionId":"value","fields":{"property":{"valueType":"value","text":["value"],"integer":["value"],"dateString":["value"],"selection":["value"],"user":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}]}}}]},"contentRestrictions":[{"readOnly":false,"reason":"value","restrictingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"restrictionTime":"value","ownerRestricted":false,"systemRestricted":false}],"downloadRestrictions":{"itemDownloadRestriction":{"restrictedForReaders":false,"restrictedForWriters":false},"effectiveDownloadRestrictionWithContext":{"restrictedForReaders":false,"restrictedForWriters":false}},"clientEncryptionDetails":{"encryptionState":"value","decryptionMetadata":{"wrappedKey":"value","kaclsId":"value"}},"copyRequiresWriterPermission":false,"writersCanShare":false,"hasThumbnail":false,"spaces":["value"],"permissionIds":["value"],"teamDriveId":"value","inheritedPermissionsDisabled":false},"removed":false,"time":"value","changeType":"value","driveId":"value","drive":{"id":"value","name":"value","hidden":false,"createdTime":"value","restrictions":{"copyRequiresWriterPermission":false,"domainUsersOnly":false,"driveMembersOnly":false,"adminManagedRestrictions":false,"sharingFoldersRequiresOrganizerPermission":false},"capabilities":{"canAddChildren":false,"canComment":false,"canCopy":false,"canDeleteDrive":false,"canDownload":false,"canEdit":false,"canListChildren":false,"canManageMembers":false,"canReadRevisions":false,"canRename":false,"canShare":false},"colorRgb":"value","backgroundImageLink":"value","orgUnitId":"value"},"changeId":"value"}"#,
        &[
            "fileId",
            "file",
            "removed",
            "time",
            "changeType",
            "driveId",
            "drive",
            "changeId",
        ],
        &[],
        &[],
    );
}

#[test]
fn googlechangelist_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleChangeList>(
        r#"{"changes":[{"fileId":"value","file":{"id":"value","name":"value","mimeType":"value","parents":["value"],"description":"value","trashed":false,"explicitlyTrashed":false,"trashedTime":"value","version":"value","createdTime":"value","modifiedTime":"value","modifiedByMeTime":"value","viewedByMeTime":"value","size":"value","quotaBytesUsed":"value","md5Checksum":"value","sha1Checksum":"value","sha256Checksum":"value","headRevisionId":"value","driveId":"value","resourceKey":"value","webViewLink":"value","webContentLink":"value","thumbnailLink":"value","iconLink":"value","fileExtension":"value","fullFileExtension":"value","originalFilename":"value","shortcutDetails":{"targetId":"value","targetMimeType":"value","targetResourceKey":"value"},"capabilities":{"canEdit":false,"canCopy":false,"canDownload":false,"canDelete":false,"canRename":false,"canMoveItemWithinDrive":false,"canMoveItemOutOfDrive":false,"canAddChildren":false,"canListChildren":false,"canShare":false,"canComment":false,"canReadRevisions":false,"canChangeCopyRequiresWriterPermission":false},"permissions":[{"id":"value","type":"value","role":"value","emailAddress":"value","domain":"value","allowFileDiscovery":false,"expirationTime":"value","displayName":"value","photoLink":"value","deleted":false,"pendingOwner":false,"permissionDetails":[{"permissionType":"value","role":"value","inherited":false,"inheritedFrom":"value"}],"view":"value"}],"owners":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}],"lastModifyingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"sharingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"shared":false,"ownedByMe":false,"modifiedByMe":false,"starred":false,"folderColorRgb":"value","properties":{"property":"value"},"appProperties":{"property":"value"},"contentHints":{"indexableText":"value","thumbnail":{"image":"value","mimeType":"value"}},"imageMediaMetadata":{"width":2,"height":2,"rotation":2,"time":"value","cameraMake":"value","cameraModel":"value","exposureTime":1.5,"aperture":1.5,"isoSpeed":2,"location":{"latitude":1.5,"longitude":1.5,"altitude":1.5}},"videoMediaMetadata":{"width":2,"height":2,"durationMillis":"value"},"labelInfo":{"labels":[{"id":"value","revisionId":"value","fields":{"property":{"valueType":"value","text":["value"],"integer":["value"],"dateString":["value"],"selection":["value"],"user":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}]}}}]},"contentRestrictions":[{"readOnly":false,"reason":"value","restrictingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"restrictionTime":"value","ownerRestricted":false,"systemRestricted":false}],"downloadRestrictions":{"itemDownloadRestriction":{"restrictedForReaders":false,"restrictedForWriters":false},"effectiveDownloadRestrictionWithContext":{"restrictedForReaders":false,"restrictedForWriters":false}},"clientEncryptionDetails":{"encryptionState":"value","decryptionMetadata":{"wrappedKey":"value","kaclsId":"value"}},"copyRequiresWriterPermission":false,"writersCanShare":false,"hasThumbnail":false,"spaces":["value"],"permissionIds":["value"],"teamDriveId":"value","inheritedPermissionsDisabled":false},"removed":false,"time":"value","changeType":"value","driveId":"value","drive":{"id":"value","name":"value","hidden":false,"createdTime":"value","restrictions":{"copyRequiresWriterPermission":false,"domainUsersOnly":false,"driveMembersOnly":false,"adminManagedRestrictions":false,"sharingFoldersRequiresOrganizerPermission":false},"capabilities":{"canAddChildren":false,"canComment":false,"canCopy":false,"canDeleteDrive":false,"canDownload":false,"canEdit":false,"canListChildren":false,"canManageMembers":false,"canReadRevisions":false,"canRename":false,"canShare":false},"colorRgb":"value","backgroundImageLink":"value","orgUnitId":"value"},"changeId":"value"}],"nextPageToken":"value","newStartPageToken":"value"}"#,
        &["nextPageToken", "newStartPageToken"],
        &["changes"],
        &[],
    );
}

#[test]
fn googlestartpagetoken_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleStartPageToken>(
        r#"{"startPageToken":"value"}"#,
        &[],
        &[],
        &["startPageToken"],
    );
}

#[test]
fn googledrive_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleDrive>(
        r#"{"id":"value","name":"value","hidden":false,"createdTime":"value","restrictions":{"copyRequiresWriterPermission":false,"domainUsersOnly":false,"driveMembersOnly":false,"adminManagedRestrictions":false,"sharingFoldersRequiresOrganizerPermission":false},"capabilities":{"canAddChildren":false,"canComment":false,"canCopy":false,"canDeleteDrive":false,"canDownload":false,"canEdit":false,"canListChildren":false,"canManageMembers":false,"canReadRevisions":false,"canRename":false,"canShare":false},"colorRgb":"value","backgroundImageLink":"value","orgUnitId":"value"}"#,
        &[
            "id",
            "name",
            "hidden",
            "createdTime",
            "restrictions",
            "capabilities",
            "colorRgb",
            "backgroundImageLink",
            "orgUnitId",
        ],
        &[],
        &[],
    );
}

#[test]
fn googledriverestrictions_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleDriveRestrictions>(
        r#"{"copyRequiresWriterPermission":false,"domainUsersOnly":false,"driveMembersOnly":false,"adminManagedRestrictions":false,"sharingFoldersRequiresOrganizerPermission":false}"#,
        &[
            "copyRequiresWriterPermission",
            "domainUsersOnly",
            "driveMembersOnly",
            "adminManagedRestrictions",
            "sharingFoldersRequiresOrganizerPermission",
        ],
        &[],
        &[],
    );
}

#[test]
fn googledrivecapabilities_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleDriveCapabilities>(
        r#"{"canAddChildren":false,"canComment":false,"canCopy":false,"canDeleteDrive":false,"canDownload":false,"canEdit":false,"canListChildren":false,"canManageMembers":false,"canReadRevisions":false,"canRename":false,"canShare":false}"#,
        &[
            "canAddChildren",
            "canComment",
            "canCopy",
            "canDeleteDrive",
            "canDownload",
            "canEdit",
            "canListChildren",
            "canManageMembers",
            "canReadRevisions",
            "canRename",
            "canShare",
        ],
        &[],
        &[],
    );
}

#[test]
fn googlepermission_fields_roundtrip_and_preserve_omission() {
    projection::<GooglePermission>(
        r#"{"id":"value","type":"value","role":"value","emailAddress":"value","domain":"value","allowFileDiscovery":false,"expirationTime":"value","displayName":"value","photoLink":"value","deleted":false,"pendingOwner":false,"permissionDetails":[{"permissionType":"value","role":"value","inherited":false,"inheritedFrom":"value"}],"view":"value"}"#,
        &[
            "id",
            "type",
            "role",
            "emailAddress",
            "domain",
            "allowFileDiscovery",
            "expirationTime",
            "displayName",
            "photoLink",
            "deleted",
            "pendingOwner",
            "permissionDetails",
            "view",
        ],
        &[],
        &[],
    );
}

#[test]
fn googlepermissiondetails_fields_roundtrip_and_preserve_omission() {
    projection::<GooglePermissionDetails>(
        r#"{"permissionType":"value","role":"value","inherited":false,"inheritedFrom":"value"}"#,
        &["permissionType", "role", "inherited", "inheritedFrom"],
        &[],
        &[],
    );
}

#[test]
fn googlerevision_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleRevision>(
        r#"{"id":"value","mimeType":"value","modifiedTime":"value","keepForever":false,"size":"value","md5Checksum":"value","published":false,"publishAuto":false,"publishedOutsideDomain":false,"originalFilename":"value","lastModifyingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"exportLinks":{"property":"value"}}"#,
        &[
            "id",
            "mimeType",
            "modifiedTime",
            "keepForever",
            "size",
            "md5Checksum",
            "published",
            "publishAuto",
            "publishedOutsideDomain",
            "originalFilename",
            "lastModifyingUser",
            "exportLinks",
        ],
        &[],
        &[],
    );
}

#[test]
fn googlecomment_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleComment>(
        r#"{"id":"value","content":"value","createdTime":"value","modifiedTime":"value","author":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"resolved":false,"deleted":false,"replies":[{"id":"value","content":"value","createdTime":"value","modifiedTime":"value","author":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"action":"value","deleted":false}],"anchor":"value","quotedFileContent":{"mimeType":"value","value":"value"}}"#,
        &[
            "id",
            "content",
            "createdTime",
            "modifiedTime",
            "author",
            "resolved",
            "deleted",
            "replies",
            "anchor",
            "quotedFileContent",
        ],
        &[],
        &[],
    );
}

#[test]
fn googlequotedcontent_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleQuotedContent>(
        r#"{"mimeType":"value","value":"value"}"#,
        &["mimeType", "value"],
        &[],
        &[],
    );
}

#[test]
fn googlereply_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleReply>(
        r#"{"id":"value","content":"value","createdTime":"value","modifiedTime":"value","author":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"action":"value","deleted":false}"#,
        &[
            "id",
            "content",
            "createdTime",
            "modifiedTime",
            "author",
            "action",
            "deleted",
        ],
        &[],
        &[],
    );
}

#[test]
fn googlechannel_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleChannel>(
        r#"{"id":"value","resourceId":"value","resourceUri":"value","expiration":"value","token":"value","type":"value","address":"value","params":{"property":"value"}}"#,
        &[
            "id",
            "resourceId",
            "resourceUri",
            "expiration",
            "token",
            "type",
            "address",
            "params",
        ],
        &[],
        &[],
    );
}

#[test]
fn googleapp_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleApp>(
        r#"{"id":"value","name":"value","objectType":"value","productUrl":"value","createUrl":"value","openUrlTemplate":"value","supportsCreate":false,"supportsImport":false,"installed":false,"authorized":false,"useByDefault":false,"primaryMimeTypes":["value"],"secondaryMimeTypes":["value"]}"#,
        &[
            "id",
            "name",
            "objectType",
            "productUrl",
            "createUrl",
            "openUrlTemplate",
            "supportsCreate",
            "supportsImport",
            "installed",
            "authorized",
            "useByDefault",
            "primaryMimeTypes",
            "secondaryMimeTypes",
        ],
        &[],
        &[],
    );
}

#[test]
fn googleaccessproposal_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleAccessProposal>(
        r#"{"fileId":"value","proposalId":"value","requesterEmailAddress":"value","recipientEmailAddress":"value","rolesAndViews":[{"role":"value","view":"value"}],"requestMessage":"value","createTime":"value"}"#,
        &[
            "fileId",
            "proposalId",
            "requesterEmailAddress",
            "recipientEmailAddress",
            "rolesAndViews",
            "requestMessage",
            "createTime",
        ],
        &[],
        &[],
    );
}

#[test]
fn googleroleandview_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleRoleAndView>(
        r#"{"role":"value","view":"value"}"#,
        &["role", "view"],
        &[],
        &[],
    );
}

#[test]
fn googleapproval_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleApproval>(
        r#"{"approvalId":"value","targetFileId":"value","createTime":"value","modifyTime":"value","completeTime":"value","dueTime":"value","status":"value","initiator":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"reviewerResponses":[{"reviewer":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"response":"value"}],"fileContentChangeBehavior":"value"}"#,
        &[
            "approvalId",
            "targetFileId",
            "createTime",
            "modifyTime",
            "completeTime",
            "dueTime",
            "status",
            "initiator",
            "reviewerResponses",
            "fileContentChangeBehavior",
        ],
        &[],
        &[],
    );
}

#[test]
fn googleapprovalreviewerresponse_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleApprovalReviewerResponse>(
        r#"{"reviewer":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"response":"value"}"#,
        &["reviewer", "response"],
        &[],
        &[],
    );
}

#[test]
fn googleoperation_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleOperation>(
        r#"{"name":"value","done":false,"metadata":{"state":"value","progressPercent":2},"response":{"downloadUri":"value","partialDownloadAllowed":false},"error":{"code":2,"message":"value","details":[{"@type":"value","reason":"value","domain":"value","metadata":{"property":"value"}}]}}"#,
        &["name", "done", "metadata", "response", "error"],
        &[],
        &[],
    );
}

#[test]
fn googleoperationmetadata_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleOperationMetadata>(
        r#"{"state":"value","progressPercent":2}"#,
        &["state", "progressPercent"],
        &[],
        &[],
    );
}

#[test]
fn googlerpcstatus_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleRpcStatus>(
        r#"{"code":2,"message":"value","details":[{"@type":"value","reason":"value","domain":"value","metadata":{"property":"value"}}]}"#,
        &["code", "message", "details"],
        &[],
        &[],
    );
}

#[test]
fn googlerpcdetail_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleRpcDetail>(
        r#"{"@type":"value","reason":"value","domain":"value","metadata":{"property":"value"}}"#,
        &["@type", "reason", "domain", "metadata"],
        &[],
        &[],
    );
}

#[test]
fn googledownloadfileresponse_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleDownloadFileResponse>(
        r#"{"downloadUri":"value","partialDownloadAllowed":false}"#,
        &["downloadUri", "partialDownloadAllowed"],
        &[],
        &[],
    );
}

#[test]
fn googlecsetoken_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleCseToken>(
        r#"{"fileId":"value","currentKaclsId":"value","currentKaclsName":"value","jwt":"value"}"#,
        &["fileId", "currentKaclsId", "currentKaclsName", "jwt"],
        &[],
        &[],
    );
}

#[test]
fn googlegeneratedids_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleGeneratedIds>(
        r#"{"ids":["value"],"space":"value"}"#,
        &["ids", "space"],
        &[],
        &[],
    );
}

#[test]
fn googlemodifylabelsresponse_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleModifyLabelsResponse>(
        r#"{"modifiedLabels":[{"id":"value","revisionId":"value","fields":{"property":{"valueType":"value","text":["value"],"integer":["value"],"dateString":["value"],"selection":["value"],"user":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}]}}}]}"#,
        &["modifiedLabels"],
        &[],
        &[],
    );
}

#[test]
fn googlefilewrite_partial_write_fields() {
    patch_field!(GoogleFileWrite, name, "name", "value");
    patch_field!(GoogleFileWrite, mime_type, "mimeType", "value");
    patch_field!(GoogleFileWrite, description, "description", "value");
    patch_field!(GoogleFileWrite, trashed, "trashed", false);
    patch_field!(GoogleFileWrite, parents, "parents", ["value"]);
    patch_field!(GoogleFileWrite, starred, "starred", false);
    patch_field!(GoogleFileWrite,properties,"properties",{"property": "value"});
    patch_field!(GoogleFileWrite,app_properties,"appProperties",{"property": "value"});
    patch_field!(
        GoogleFileWrite,
        copy_requires_writer_permission,
        "copyRequiresWriterPermission",
        false
    );
    patch_field!(GoogleFileWrite, folder_color_rgb, "folderColorRgb", "value");
    patch_field!(GoogleFileWrite, modified_time, "modifiedTime", "value");
}

#[test]
fn googledrivewrite_partial_write_fields() {
    patch_field!(GoogleDriveWrite, name, "name", "value");
    patch_field!(GoogleDriveWrite,restrictions,"restrictions",{"copyRequiresWriterPermission": false, "domainUsersOnly": false, "driveMembersOnly": false, "adminManagedRestrictions": false, "sharingFoldersRequiresOrganizerPermission": false});
    patch_field!(GoogleDriveWrite, color_rgb, "colorRgb", "value");
}

#[test]
fn googlepermissionwrite_partial_write_fields() {
    patch_field!(GooglePermissionWrite, type_, "type", "value");
    patch_field!(GooglePermissionWrite, role, "role", "value");
    patch_field!(
        GooglePermissionWrite,
        email_address,
        "emailAddress",
        "value"
    );
    patch_field!(GooglePermissionWrite, domain, "domain", "value");
    patch_field!(
        GooglePermissionWrite,
        allow_file_discovery,
        "allowFileDiscovery",
        false
    );
    patch_field!(
        GooglePermissionWrite,
        expiration_time,
        "expirationTime",
        "value"
    );
    patch_field!(GooglePermissionWrite, view, "view", "value");
}

#[test]
fn googlecommentwrite_partial_write_fields() {
    patch_field!(GoogleCommentWrite, content, "content", "value");
    patch_field!(GoogleCommentWrite, anchor, "anchor", "value");
}

#[test]
fn googlereplywrite_partial_write_fields() {
    patch_field!(GoogleReplyWrite, content, "content", "value");
    patch_field!(GoogleReplyWrite, action, "action", "value");
}

#[test]
fn googlerevisionwrite_partial_write_fields() {
    patch_field!(GoogleRevisionWrite, keep_forever, "keepForever", false);
    patch_field!(GoogleRevisionWrite, published, "published", false);
    patch_field!(GoogleRevisionWrite, publish_auto, "publishAuto", false);
    patch_field!(
        GoogleRevisionWrite,
        published_outside_domain,
        "publishedOutsideDomain",
        false
    );
}

#[test]
fn googleaccessproposallist_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleAccessProposalList>(
        r#"{"accessProposals":[{"fileId":"value","proposalId":"value","requesterEmailAddress":"value","recipientEmailAddress":"value","rolesAndViews":[{"role":"value","view":"value"}],"requestMessage":"value","createTime":"value"}],"nextPageToken":"value"}"#,
        &["nextPageToken"],
        &["accessProposals"],
        &[],
    );
}

#[test]
fn googleapplist_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleAppList>(
        r#"{"items":[{"id":"value","name":"value","objectType":"value","productUrl":"value","createUrl":"value","openUrlTemplate":"value","supportsCreate":false,"supportsImport":false,"installed":false,"authorized":false,"useByDefault":false,"primaryMimeTypes":["value"],"secondaryMimeTypes":["value"]}],"nextPageToken":"value"}"#,
        &["nextPageToken"],
        &["items"],
        &[],
    );
}

#[test]
fn googleapprovallist_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleApprovalList>(
        r#"{"items":[{"approvalId":"value","targetFileId":"value","createTime":"value","modifyTime":"value","completeTime":"value","dueTime":"value","status":"value","initiator":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"reviewerResponses":[{"reviewer":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"response":"value"}],"fileContentChangeBehavior":"value"}],"nextPageToken":"value"}"#,
        &["nextPageToken"],
        &["items"],
        &[],
    );
}

#[test]
fn googlecommentlist_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleCommentList>(
        r#"{"comments":[{"id":"value","content":"value","createdTime":"value","modifiedTime":"value","author":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"resolved":false,"deleted":false,"replies":[{"id":"value","content":"value","createdTime":"value","modifiedTime":"value","author":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"action":"value","deleted":false}],"anchor":"value","quotedFileContent":{"mimeType":"value","value":"value"}}],"nextPageToken":"value"}"#,
        &["nextPageToken"],
        &["comments"],
        &[],
    );
}

#[test]
fn googledrivelist_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleDriveList>(
        r#"{"drives":[{"id":"value","name":"value","hidden":false,"createdTime":"value","restrictions":{"copyRequiresWriterPermission":false,"domainUsersOnly":false,"driveMembersOnly":false,"adminManagedRestrictions":false,"sharingFoldersRequiresOrganizerPermission":false},"capabilities":{"canAddChildren":false,"canComment":false,"canCopy":false,"canDeleteDrive":false,"canDownload":false,"canEdit":false,"canListChildren":false,"canManageMembers":false,"canReadRevisions":false,"canRename":false,"canShare":false},"colorRgb":"value","backgroundImageLink":"value","orgUnitId":"value"}],"nextPageToken":"value"}"#,
        &["nextPageToken"],
        &["drives"],
        &[],
    );
}

#[test]
fn googlelabellist_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleLabelList>(
        r#"{"labels":[{"id":"value","revisionId":"value","fields":{"property":{"valueType":"value","text":["value"],"integer":["value"],"dateString":["value"],"selection":["value"],"user":[{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false}]}}}],"nextPageToken":"value"}"#,
        &["nextPageToken"],
        &["labels"],
        &[],
    );
}

#[test]
fn googlepermissionlist_fields_roundtrip_and_preserve_omission() {
    projection::<GooglePermissionList>(
        r#"{"permissions":[{"id":"value","type":"value","role":"value","emailAddress":"value","domain":"value","allowFileDiscovery":false,"expirationTime":"value","displayName":"value","photoLink":"value","deleted":false,"pendingOwner":false,"permissionDetails":[{"permissionType":"value","role":"value","inherited":false,"inheritedFrom":"value"}],"view":"value"}],"nextPageToken":"value"}"#,
        &["nextPageToken"],
        &["permissions"],
        &[],
    );
}

#[test]
fn googlereplylist_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleReplyList>(
        r#"{"replies":[{"id":"value","content":"value","createdTime":"value","modifiedTime":"value","author":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"action":"value","deleted":false}],"nextPageToken":"value"}"#,
        &["nextPageToken"],
        &["replies"],
        &[],
    );
}

#[test]
fn googlerevisionlist_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleRevisionList>(
        r#"{"revisions":[{"id":"value","mimeType":"value","modifiedTime":"value","keepForever":false,"size":"value","md5Checksum":"value","published":false,"publishAuto":false,"publishedOutsideDomain":false,"originalFilename":"value","lastModifyingUser":{"displayName":"value","emailAddress":"value","photoLink":"value","permissionId":"value","me":false},"exportLinks":{"property":"value"}}],"nextPageToken":"value"}"#,
        &["nextPageToken"],
        &["revisions"],
        &[],
    );
}

#[test]
fn aboutoptions_fields_roundtrip_and_preserve_omission() {
    projection::<AboutOptions>(r#"{"fields":"value"}"#, &["fields"], &[], &[]);
}

#[test]
fn applistoptions_fields_roundtrip_and_preserve_omission() {
    projection::<AppListOptions>(
        r#"{"appFilterExtensions":["value"],"appFilterMimeTypes":["value"],"languageCode":"value","fields":"value"}"#,
        &[
            "appFilterExtensions",
            "appFilterMimeTypes",
            "languageCode",
            "fields",
        ],
        &[],
        &[],
    );
}

#[test]
fn changescopeoptions_fields_roundtrip_and_preserve_omission() {
    projection::<ChangeScopeOptions>(
        r#"{"driveId":"value","supportsAllDrives":false}"#,
        &["driveId", "supportsAllDrives"],
        &[],
        &[],
    );
}

#[test]
fn changelistoptions_fields_roundtrip_and_preserve_omission() {
    projection::<ChangeListOptions>(
        r#"{"pageToken":"value","driveId":"value","pageSize":2,"spaces":"value","includeRemoved":false,"includeCorpusRemovals":false,"includeItemsFromAllDrives":false,"supportsAllDrives":false,"fields":"value"}"#,
        &[
            "driveId",
            "pageSize",
            "spaces",
            "includeRemoved",
            "includeCorpusRemovals",
            "includeItemsFromAllDrives",
            "supportsAllDrives",
            "fields",
        ],
        &[],
        &["pageToken"],
    );
}

#[test]
fn changewatchoptions_fields_roundtrip_and_preserve_omission() {
    projection::<ChangeWatchOptions>(
        r#"{"pageToken":"value","driveId":"value","pageSize":2,"spaces":"value","includeRemoved":false,"includeItemsFromAllDrives":false,"supportsAllDrives":false,"fields":"value"}"#,
        &[
            "driveId",
            "pageSize",
            "spaces",
            "includeRemoved",
            "includeItemsFromAllDrives",
            "supportsAllDrives",
            "fields",
        ],
        &[],
        &["pageToken"],
    );
}

#[test]
fn commentoptions_fields_roundtrip_and_preserve_omission() {
    projection::<CommentOptions>(
        r#"{"fields":"value","includeDeleted":false}"#,
        &["fields", "includeDeleted"],
        &[],
        &[],
    );
}

#[test]
fn commentlistoptions_fields_roundtrip_and_preserve_omission() {
    projection::<CommentListOptions>(
        r#"{"fields":"value","pageSize":2,"pageToken":"value","includeDeleted":false,"startModifiedTime":"value"}"#,
        &[
            "fields",
            "pageSize",
            "pageToken",
            "includeDeleted",
            "startModifiedTime",
        ],
        &[],
        &[],
    );
}

#[test]
fn csetokenoptions_fields_roundtrip_and_preserve_omission() {
    projection::<CseTokenOptions>(
        r#"{"fileId":"value","parent":"value"}"#,
        &["fileId", "parent"],
        &[],
        &[],
    );
}

#[test]
fn drivedeleteoptions_fields_roundtrip_and_preserve_omission() {
    projection::<DriveDeleteOptions>(
        r#"{"allowItemDeletion":false,"useDomainAdminAccess":false}"#,
        &["allowItemDeletion", "useDomainAdminAccess"],
        &[],
        &[],
    );
}

#[test]
fn drivegetoptions_fields_roundtrip_and_preserve_omission() {
    projection::<DriveGetOptions>(
        r#"{"useDomainAdminAccess":false,"fields":"value"}"#,
        &["useDomainAdminAccess", "fields"],
        &[],
        &[],
    );
}

#[test]
fn drivelistoptions_fields_roundtrip_and_preserve_omission() {
    projection::<DriveListOptions>(
        r#"{"pageSize":2,"pageToken":"value","q":"value","useDomainAdminAccess":false,"fields":"value"}"#,
        &[
            "pageSize",
            "pageToken",
            "q",
            "useDomainAdminAccess",
            "fields",
        ],
        &[],
        &[],
    );
}

#[test]
fn driveupdateoptions_fields_roundtrip_and_preserve_omission() {
    projection::<DriveUpdateOptions>(
        r#"{"useDomainAdminAccess":false,"fields":"value"}"#,
        &["useDomainAdminAccess", "fields"],
        &[],
        &[],
    );
}

#[test]
fn emptytrashoptions_fields_roundtrip_and_preserve_omission() {
    projection::<EmptyTrashOptions>(r#"{"driveId":"value"}"#, &["driveId"], &[], &[]);
}

#[test]
fn filecopyoptions_fields_roundtrip_and_preserve_omission() {
    projection::<FileCopyOptions>(
        r#"{"supportsAllDrives":false,"ignoreDefaultVisibility":false,"fields":"value"}"#,
        &["supportsAllDrives", "ignoreDefaultVisibility", "fields"],
        &[],
        &[],
    );
}

#[test]
fn filecreateoptions_fields_roundtrip_and_preserve_omission() {
    projection::<FileCreateOptions>(
        r#"{"supportsAllDrives":false,"ignoreDefaultVisibility":false,"useContentAsIndexableText":false,"keepRevisionForever":false,"fields":"value"}"#,
        &[
            "supportsAllDrives",
            "ignoreDefaultVisibility",
            "useContentAsIndexableText",
            "keepRevisionForever",
            "fields",
        ],
        &[],
        &[],
    );
}

#[test]
fn filedeleteoptions_fields_roundtrip_and_preserve_omission() {
    projection::<FileDeleteOptions>(
        r#"{"supportsAllDrives":false,"enforceSingleParent":false}"#,
        &["supportsAllDrives", "enforceSingleParent"],
        &[],
        &[],
    );
}

#[test]
fn filedownloadoptions_fields_roundtrip_and_preserve_omission() {
    projection::<FileDownloadOptions>(
        r#"{"mimeType":"value","revisionId":"value"}"#,
        &["mimeType", "revisionId"],
        &[],
        &[],
    );
}

#[test]
fn filegetoptions_fields_roundtrip_and_preserve_omission() {
    projection::<FileGetOptions>(
        r#"{"fields":"value","supportsAllDrives":false,"includeLabels":"value","includePermissionsForView":"value"}"#,
        &[
            "fields",
            "supportsAllDrives",
            "includeLabels",
            "includePermissionsForView",
        ],
        &[],
        &[],
    );
}

#[test]
fn filelistoptions_fields_roundtrip_and_preserve_omission() {
    projection::<FileListOptions>(
        r#"{"q":"value","corpora":"value","driveId":"value","pageSize":2,"pageToken":"value","spaces":"value","orderBy":"value","fields":"value","includeItemsFromAllDrives":false,"supportsAllDrives":false,"includePermissionsForView":"value"}"#,
        &[
            "q",
            "corpora",
            "driveId",
            "pageSize",
            "pageToken",
            "spaces",
            "orderBy",
            "fields",
            "includeItemsFromAllDrives",
            "supportsAllDrives",
            "includePermissionsForView",
        ],
        &[],
        &[],
    );
}

#[test]
fn fileupdateoptions_fields_roundtrip_and_preserve_omission() {
    projection::<FileUpdateOptions>(
        r#"{"addParents":["value"],"removeParents":["value"],"supportsAllDrives":false,"keepRevisionForever":false,"useContentAsIndexableText":false,"fields":"value"}"#,
        &[
            "addParents",
            "removeParents",
            "supportsAllDrives",
            "keepRevisionForever",
            "useContentAsIndexableText",
            "fields",
        ],
        &[],
        &[],
    );
}

#[test]
fn filewatchoptions_fields_roundtrip_and_preserve_omission() {
    projection::<FileWatchOptions>(
        r#"{"acknowledgeAbuse":false,"supportsAllDrives":false}"#,
        &["acknowledgeAbuse", "supportsAllDrives"],
        &[],
        &[],
    );
}

#[test]
fn generateidsoptions_fields_roundtrip_and_preserve_omission() {
    projection::<GenerateIdsOptions>(
        r#"{"count":2,"space":"value","type":"value"}"#,
        &["count", "space", "type"],
        &[],
        &[],
    );
}

#[test]
fn labellistoptions_fields_roundtrip_and_preserve_omission() {
    projection::<LabelListOptions>(
        r#"{"pageSize":2,"pageToken":"value"}"#,
        &["pageSize", "pageToken"],
        &[],
        &[],
    );
}

#[test]
fn permissioncreateoptions_fields_roundtrip_and_preserve_omission() {
    projection::<PermissionCreateOptions>(
        r#"{"sendNotificationEmail":false,"emailMessage":"value","supportsAllDrives":false,"transferOwnership":false,"useDomainAdminAccess":false,"fields":"value"}"#,
        &[
            "sendNotificationEmail",
            "emailMessage",
            "supportsAllDrives",
            "transferOwnership",
            "useDomainAdminAccess",
            "fields",
        ],
        &[],
        &[],
    );
}

#[test]
fn permissiondeleteoptions_fields_roundtrip_and_preserve_omission() {
    projection::<PermissionDeleteOptions>(
        r#"{"supportsAllDrives":false,"useDomainAdminAccess":false}"#,
        &["supportsAllDrives", "useDomainAdminAccess"],
        &[],
        &[],
    );
}

#[test]
fn permissiongetoptions_fields_roundtrip_and_preserve_omission() {
    projection::<PermissionGetOptions>(
        r#"{"supportsAllDrives":false,"useDomainAdminAccess":false,"fields":"value"}"#,
        &["supportsAllDrives", "useDomainAdminAccess", "fields"],
        &[],
        &[],
    );
}

#[test]
fn permissionlistoptions_fields_roundtrip_and_preserve_omission() {
    projection::<PermissionListOptions>(
        r#"{"supportsAllDrives":false,"useDomainAdminAccess":false,"pageSize":2,"pageToken":"value","fields":"value"}"#,
        &[
            "supportsAllDrives",
            "useDomainAdminAccess",
            "pageSize",
            "pageToken",
            "fields",
        ],
        &[],
        &[],
    );
}

#[test]
fn permissionupdateoptions_fields_roundtrip_and_preserve_omission() {
    projection::<PermissionUpdateOptions>(
        r#"{"supportsAllDrives":false,"useDomainAdminAccess":false,"removeExpiration":false,"transferOwnership":false,"fields":"value"}"#,
        &[
            "supportsAllDrives",
            "useDomainAdminAccess",
            "removeExpiration",
            "transferOwnership",
            "fields",
        ],
        &[],
        &[],
    );
}

#[test]
fn replyoptions_fields_roundtrip_and_preserve_omission() {
    projection::<ReplyOptions>(
        r#"{"fields":"value","includeDeleted":false}"#,
        &["fields", "includeDeleted"],
        &[],
        &[],
    );
}

#[test]
fn replylistoptions_fields_roundtrip_and_preserve_omission() {
    projection::<ReplyListOptions>(
        r#"{"fields":"value","pageSize":2,"pageToken":"value","includeDeleted":false}"#,
        &["fields", "pageSize", "pageToken", "includeDeleted"],
        &[],
        &[],
    );
}

#[test]
fn revisiongetoptions_fields_roundtrip_and_preserve_omission() {
    projection::<RevisionGetOptions>(
        r#"{"fields":"value","acknowledgeAbuse":false}"#,
        &["fields", "acknowledgeAbuse"],
        &[],
        &[],
    );
}

#[test]
fn revisionlistoptions_fields_roundtrip_and_preserve_omission() {
    projection::<RevisionListOptions>(
        r#"{"fields":"value","pageSize":2,"pageToken":"value"}"#,
        &["fields", "pageSize", "pageToken"],
        &[],
        &[],
    );
}

#[test]
fn googlechannelrequest_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleChannelRequest>(
        r#"{"id":"value","type":"value","address":"value","token":"value","expiration":"value","params":{"property":"value"}}"#,
        &["token", "expiration", "params"],
        &[],
        &["id", "type", "address"],
    );
}

#[test]
fn googlechannelstoprequest_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleChannelStopRequest>(
        r#"{"id":"value","resourceId":"value"}"#,
        &[],
        &[],
        &["id", "resourceId"],
    );
}

#[test]
fn googlemodifylabelsrequest_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleModifyLabelsRequest>(
        r#"{"labelModifications":[{"labelId":"value","fieldModifications":[{"fieldId":"value","setTextValues":["value"],"setIntegerValues":["value"],"setDateValues":["value"],"setSelectionValues":["value"],"unsetValues":false}],"removeLabel":false}]}"#,
        &[],
        &["labelModifications"],
        &[],
    );
}

#[test]
fn googlelabelmodification_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleLabelModification>(
        r#"{"labelId":"value","fieldModifications":[{"fieldId":"value","setTextValues":["value"],"setIntegerValues":["value"],"setDateValues":["value"],"setSelectionValues":["value"],"unsetValues":false}],"removeLabel":false}"#,
        &["removeLabel"],
        &["fieldModifications"],
        &["labelId"],
    );
}

#[test]
fn googlelabelfieldmodification_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleLabelFieldModification>(
        r#"{"fieldId":"value","setTextValues":["value"],"setIntegerValues":["value"],"setDateValues":["value"],"setSelectionValues":["value"],"unsetValues":false}"#,
        &[
            "setTextValues",
            "setIntegerValues",
            "setDateValues",
            "setSelectionValues",
            "unsetValues",
        ],
        &[],
        &["fieldId"],
    );
}

#[test]
fn approveapprovalrequest_fields_roundtrip_and_preserve_omission() {
    projection::<ApproveApprovalRequest>(r#"{"message":"value"}"#, &["message"], &[], &[]);
}

#[test]
fn cancelapprovalrequest_fields_roundtrip_and_preserve_omission() {
    projection::<CancelApprovalRequest>(r#"{"message":"value"}"#, &["message"], &[], &[]);
}

#[test]
fn commentapprovalrequest_fields_roundtrip_and_preserve_omission() {
    projection::<CommentApprovalRequest>(r#"{"message":"value"}"#, &[], &[], &["message"]);
}

#[test]
fn declineapprovalrequest_fields_roundtrip_and_preserve_omission() {
    projection::<DeclineApprovalRequest>(r#"{"message":"value"}"#, &["message"], &[], &[]);
}

#[test]
fn reassignapprovalrequest_fields_roundtrip_and_preserve_omission() {
    projection::<ReassignApprovalRequest>(
        r#"{"replaceReviewers":[{"removedReviewerEmail":"value","addedReviewerEmail":"value"}],"addReviewers":[{"addedReviewerEmail":"value"}],"message":"value"}"#,
        &["replaceReviewers", "addReviewers", "message"],
        &[],
        &[],
    );
}

#[test]
fn googlereplacereviewer_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleReplaceReviewer>(
        r#"{"removedReviewerEmail":"value","addedReviewerEmail":"value"}"#,
        &[],
        &[],
        &["removedReviewerEmail", "addedReviewerEmail"],
    );
}

#[test]
fn googleaddreviewer_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleAddReviewer>(
        r#"{"addedReviewerEmail":"value"}"#,
        &[],
        &[],
        &["addedReviewerEmail"],
    );
}

#[test]
fn startapprovalrequest_fields_roundtrip_and_preserve_omission() {
    projection::<StartApprovalRequest>(
        r#"{"reviewerEmails":["value"],"lockFile":false,"message":"value","dueTime":"value","fileContentChangeBehavior":"value"}"#,
        &[
            "lockFile",
            "message",
            "dueTime",
            "fileContentChangeBehavior",
        ],
        &["reviewerEmails"],
        &[],
    );
}

#[test]
fn googleapprovalreviewer_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleApprovalReviewer>(
        r#"{"emailAddress":"value"}"#,
        &[],
        &[],
        &["emailAddress"],
    );
}

#[test]
fn resolveaccessproposalrequest_fields_roundtrip_and_preserve_omission() {
    projection::<ResolveAccessProposalRequest>(
        r#"{"action":"value","role":["value"],"view":"value","sendNotification":false}"#,
        &["role", "view", "sendNotification"],
        &[],
        &["action"],
    );
}

#[test]
fn googleconfigsources_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleConfigSources>(
        r#"{"client_id_env":"value","client_secret_env":"value","oauth_file_env":"value","bundled_config_path":"value"}"#,
        &[
            "client_id_env",
            "client_secret_env",
            "oauth_file_env",
            "bundled_config_path",
        ],
        &[],
        &[],
    );
}

#[test]
fn googleoauthconfig_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleOAuthConfig>(
        r#"{"client_id":"value","client_secret":"value","redirect_uris":["value"],"scopes":["value"]}"#,
        &["client_secret"],
        &["redirect_uris", "scopes"],
        &["client_id"],
    );
}

#[test]
fn googleauthorizationrequest_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleAuthorizationRequest>(
        r#"{"redirect_uri":"value","state":"value","code_challenge":"value","code_challenge_method":"value","scopes":["value"],"access_type":"value","prompt":"value","login_hint":"value"}"#,
        &["access_type", "prompt", "login_hint"],
        &["scopes"],
        &[
            "redirect_uri",
            "state",
            "code_challenge",
            "code_challenge_method",
        ],
    );
}

#[test]
fn googleauthorizationurl_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleAuthorizationUrl>(r#"{"url":"value"}"#, &[], &[], &["url"]);
}

#[test]
fn googletokenresponse_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleTokenResponse>(
        r#"{"access_token":"value","token_type":"value","expires_in":2,"refresh_token":"value","scope":"value","id_token":"value","refresh_token_expires_in":2}"#,
        &[
            "refresh_token",
            "scope",
            "id_token",
            "refresh_token_expires_in",
        ],
        &[],
        &["access_token", "token_type", "expires_in"],
    );
}

#[test]
fn googleuserinfo_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleUserInfo>(
        r#"{"sub":"value","email":"value","email_verified":false,"name":"value","picture":"value","hd":"value"}"#,
        &["email", "email_verified", "name", "picture", "hd"],
        &[],
        &["sub"],
    );
}

#[test]
fn googleidtokenvalidation_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleIdTokenValidation>(
        r#"{"issuer":"value","audience":"value","nonce":"value","clock_skew_seconds":2}"#,
        &["nonce", "clock_skew_seconds"],
        &[],
        &["issuer", "audience"],
    );
}

#[test]
fn googleidclaims_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleIdClaims>(
        r#"{"iss":"value","aud":"client-id","azp":"value","sub":"value","exp":2,"iat":2,"email":"value","email_verified":false,"nonce":"value"}"#,
        &["azp", "email", "email_verified", "nonce"],
        &[],
        &["iss", "aud", "sub", "exp", "iat"],
    );
}

#[test]
fn googleoidcdiscovery_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleOidcDiscovery>(
        r#"{"issuer":"value","authorization_endpoint":"value","token_endpoint":"value","userinfo_endpoint":"value","jwks_uri":"value","revocation_endpoint":"value","scopes_supported":["value"]}"#,
        &["revocation_endpoint", "scopes_supported"],
        &[],
        &[
            "issuer",
            "authorization_endpoint",
            "token_endpoint",
            "userinfo_endpoint",
            "jwks_uri",
        ],
    );
}

#[test]
fn googlejwkset_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleJwkSet>(
        r#"{"keys":[{"kid":"value","kty":"value","alg":"value","use":"value","n":"value","e":"value","crv":"value","x":"value","y":"value"}]}"#,
        &[],
        &["keys"],
        &[],
    );
}

#[test]
fn googlejwk_fields_roundtrip_and_preserve_omission() {
    projection::<GoogleJwk>(
        r#"{"kid":"value","kty":"value","alg":"value","use":"value","n":"value","e":"value","crv":"value","x":"value","y":"value"}"#,
        &["kid", "alg", "use", "n", "e", "crv", "x", "y"],
        &[],
        &["kty"],
    );
}
