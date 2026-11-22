ALTER TABLE local_site
    ALTER registration_mode DROP DEFAULT;

UPDATE local_site
SET registration_mode = 'Closed'
WHERE registration_mode = 'RequirePassport';

ALTER TYPE registration_mode_enum RENAME TO registration_mode_enum__;

CREATE TYPE registration_mode_enum AS ENUM (
    'Closed',
    'RequireApplication',
    'Open'
);

ALTER TABLE local_site
    ALTER COLUMN registration_mode TYPE registration_mode_enum
    USING registration_mode::text::registration_mode_enum;

ALTER TABLE local_site
    ALTER registration_mode SET DEFAULT 'RequireApplication';

DROP TYPE registration_mode_enum__;
