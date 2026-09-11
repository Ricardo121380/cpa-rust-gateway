CREATE TABLE configuration_edit_origins (
    config_version_id TEXT PRIMARY KEY REFERENCES config_versions(id) ON DELETE CASCADE,
    source_version_id TEXT NOT NULL REFERENCES config_versions(id),
    source_revision INTEGER NOT NULL CHECK(source_revision >= 0),
    source_resource_sequence INTEGER NOT NULL CHECK(source_resource_sequence >= 0),
    source_lifecycle_sequence INTEGER NOT NULL CHECK(source_lifecycle_sequence >= 0),
    CHECK(config_version_id <> source_version_id)
) STRICT;
