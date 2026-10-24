-- Face capture poll UX: expose in-flight pipeline stage to clients.

ALTER TABLE face_capture_pending
    ADD COLUMN pipeline_stage VARCHAR(32) NULL COMMENT 'queued|validating|likeness|painting' AFTER status;
