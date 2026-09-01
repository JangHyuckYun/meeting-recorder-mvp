ALTER TABLE model_assignments ADD COLUMN reasoning_effort TEXT;
ALTER TABLE model_assignments ADD COLUMN fast INTEGER NOT NULL DEFAULT 0;
UPDATE providers SET models_json = '["gpt-5.6-sol","gpt-5.6-terra","gpt-5.6-luna","gpt-5.5","gpt-5.4","gpt-5.4-mini","gpt-5.4-nano","gpt-5.1","gpt-5.1-codex","gpt-5.1-codex-max"]' WHERE id = '00000000-0000-0000-0000-000000000001';
UPDATE providers SET models_json = '["claude-fable-5","claude-opus-5","claude-opus-4-8","claude-opus-4-7","claude-opus-4-6","claude-sonnet-5","claude-sonnet-4-6","claude-haiku-4-5"]' WHERE id = '00000000-0000-0000-0000-000000000002';
UPDATE model_assignments SET model_name = 'gpt-5.6-terra' WHERE provider_id = '00000000-0000-0000-0000-000000000001' AND model_name IN ('gpt-4o','gpt-4.1-mini');
