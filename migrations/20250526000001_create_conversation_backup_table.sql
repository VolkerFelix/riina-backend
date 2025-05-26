-- Optional: Create PostgreSQL tables for conversation backup/analytics
-- Primary storage is Redis, but this provides persistence and analytics

CREATE TABLE IF NOT EXISTS conversation_contexts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    relationship_stage VARCHAR(50) NOT NULL,
    total_messages INTEGER NOT NULL DEFAULT 0,
    engagement_level DECIMAL(3,2) NOT NULL DEFAULT 0.50,
    trust_level DECIMAL(3,2) NOT NULL DEFAULT 0.50,
    humor_level INTEGER NOT NULL DEFAULT 7,
    empathy_level INTEGER NOT NULL DEFAULT 7,
    sarcasm_level INTEGER NOT NULL DEFAULT 4,
    encouragement_style VARCHAR(50) NOT NULL DEFAULT 'supportive',
    conversation_style VARCHAR(50) NOT NULL DEFAULT 'casual',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_updated TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    context_data JSONB NOT NULL DEFAULT '{}'
);

CREATE TABLE IF NOT EXISTS conversation_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    conversation_id UUID REFERENCES conversation_contexts(id) ON DELETE CASCADE,
    message_type VARCHAR(50) NOT NULL, -- 'twin_thought', 'user_response', 'reaction', etc.
    sender VARCHAR(20) NOT NULL, -- 'twin', 'user', 'system'
    content TEXT NOT NULL,
    mood VARCHAR(50),
    context_tags TEXT[], -- Array of tags like ['humor', 'health', 'encouragement']
    user_reaction JSONB, -- UserReaction struct as JSON
    twin_confidence DECIMAL(3,2),
    message_intent VARCHAR(50),
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS personality_changes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    trait_name VARCHAR(50) NOT NULL,
    old_value INTEGER NOT NULL,
    new_value INTEGER NOT NULL,
    trigger_reason TEXT NOT NULL,
    confidence DECIMAL(3,2) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_conversation_contexts_user_id ON conversation_contexts(user_id);
CREATE INDEX IF NOT EXISTS idx_conversation_contexts_updated ON conversation_contexts(last_updated);

CREATE INDEX IF NOT EXISTS idx_conversation_messages_user_id ON conversation_messages(user_id);
CREATE INDEX IF NOT EXISTS idx_conversation_messages_created_at ON conversation_messages(created_at);
CREATE INDEX IF NOT EXISTS idx_conversation_messages_type ON conversation_messages(message_type);
CREATE INDEX IF NOT EXISTS idx_conversation_messages_sender ON conversation_messages(sender);

CREATE INDEX IF NOT EXISTS idx_personality_changes_user_id ON personality_changes(user_id);
CREATE INDEX IF NOT EXISTS idx_personality_changes_trait ON personality_changes(trait_name);
CREATE INDEX IF NOT EXISTS idx_personality_changes_created_at ON personality_changes(created_at);

-- Function to backup conversation context from Redis to PostgreSQL
CREATE OR REPLACE FUNCTION backup_conversation_context(
    p_user_id UUID,
    p_context_data JSONB
) RETURNS UUID AS $$
DECLARE
    context_id UUID;
BEGIN
    INSERT INTO conversation_contexts (
        user_id,
        relationship_stage,
        total_messages,
        engagement_level,
        trust_level,
        humor_level,
        empathy_level,
        sarcasm_level,
        encouragement_style,
        conversation_style,
        context_data
    ) VALUES (
        p_user_id,
        COALESCE((p_context_data->>'relationship_stage')::VARCHAR, 'first_meeting'),
        COALESCE((p_context_data->>'total_messages')::INTEGER, 0),
        COALESCE((p_context_data->>'engagement_level')::DECIMAL, 0.50),
        COALESCE((p_context_data->>'trust_level')::DECIMAL, 0.50),
        COALESCE((p_context_data->>'humor_level')::INTEGER, 7),
        COALESCE((p_context_data->>'empathy_level')::INTEGER, 7),
        COALESCE((p_context_data->>'sarcasm_level')::INTEGER, 4),
        COALESCE((p_context_data->>'encouragement_style')::VARCHAR, 'supportive'),
        COALESCE((p_context_data->>'conversation_style')::VARCHAR, 'casual'),
        p_context_data
    )
    ON CONFLICT (user_id) DO UPDATE SET
        relationship_stage = EXCLUDED.relationship_stage,
        total_messages = EXCLUDED.total_messages,
        engagement_level = EXCLUDED.engagement_level,
        trust_level = EXCLUDED.trust_level,
        humor_level = EXCLUDED.humor_level,
        empathy_level = EXCLUDED.empathy_level,
        sarcasm_level = EXCLUDED.sarcasm_level,
        encouragement_style = EXCLUDED.encouragement_style,
        conversation_style = EXCLUDED.conversation_style,
        context_data = EXCLUDED.context_data,
        last_updated = NOW()
    RETURNING id INTO context_id;
    
    RETURN context_id;
END;
$$ LANGUAGE plpgsql;

-- Add unique constraint for user_id
ALTER TABLE conversation_contexts ADD CONSTRAINT unique_conversation_context_user_id UNIQUE (user_id);