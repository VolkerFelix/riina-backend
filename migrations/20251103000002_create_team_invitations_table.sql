-- Create team_invitations table
CREATE TABLE team_invitations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    inviter_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    invitee_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    responded_at TIMESTAMPTZ,
    CONSTRAINT valid_status CHECK (status IN ('pending', 'accepted', 'declined', 'expired'))
);

-- Create indexes for efficient querying
CREATE INDEX idx_team_invitations_invitee ON team_invitations(invitee_id, status);
CREATE INDEX idx_team_invitations_team ON team_invitations(team_id, status);
CREATE INDEX idx_team_invitations_created_at ON team_invitations(created_at);

-- Add comment
COMMENT ON TABLE team_invitations IS 'Stores team recruitment invitations sent to free agents';
