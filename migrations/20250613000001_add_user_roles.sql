-- Add system-level roles and status to users table
ALTER TABLE users 
ADD COLUMN role VARCHAR(20) NOT NULL DEFAULT 'user',
ADD COLUMN status VARCHAR(20) NOT NULL DEFAULT 'active';

-- Add constraints for valid role and status values
ALTER TABLE users 
ADD CONSTRAINT valid_user_role 
    CHECK (role IN ('superadmin', 'admin', 'moderator', 'user'));

ALTER TABLE users 
ADD CONSTRAINT valid_user_status 
    CHECK (status IN ('active', 'inactive', 'suspended', 'banned'));

-- Create index for efficient role-based queries
CREATE INDEX idx_users_role ON users(role);
CREATE INDEX idx_users_status ON users(status);