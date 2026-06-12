-- Add ownerId column to CheckinAccount
ALTER TABLE CheckinAccount ADD COLUMN ownerId TEXT;

-- Set existing accounts to admin user (first user in DB)
UPDATE CheckinAccount SET ownerId = (SELECT id FROM AppUser ORDER BY createdAt LIMIT 1) WHERE ownerId IS NULL;