-- Create team_polls table for member removal polls
CREATE TABLE team_polls (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    poll_type TEXT NOT NULL CHECK (poll_type IN ('member_removal')),
    target_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'completed', 'expired', 'cancelled')),
    result TEXT CHECK (result IN ('approved', 'rejected', 'no_consensus')),
    executed_at TIMESTAMPTZ,

    -- Constraints
    CONSTRAINT expires_after_created CHECK (expires_at > created_at),
    CONSTRAINT result_when_not_active CHECK (
        (status = 'active' AND result IS NULL AND executed_at IS NULL) OR
        (status != 'active')
    )
);

-- Create poll_votes table to track individual votes
CREATE TABLE poll_votes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    poll_id UUID NOT NULL REFERENCES team_polls(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    vote TEXT NOT NULL CHECK (vote IN ('for', 'against')),
    voted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- One vote per user per poll
    CONSTRAINT unique_vote_per_poll UNIQUE (poll_id, user_id)
);

-- Indexes for performance
CREATE INDEX idx_team_polls_team_id ON team_polls(team_id);
CREATE INDEX idx_team_polls_status ON team_polls(status);
CREATE INDEX idx_team_polls_target_user ON team_polls(target_user_id);
CREATE INDEX idx_team_polls_expires_at ON team_polls(expires_at) WHERE status = 'active';
CREATE INDEX idx_poll_votes_poll_id ON poll_votes(poll_id);
CREATE INDEX idx_poll_votes_user_id ON poll_votes(user_id);
