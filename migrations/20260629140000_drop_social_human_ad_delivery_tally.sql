-- Feed ad accrual removed; tally column no longer used.
ALTER TABLE social_human
    DROP CHECK chk_social_human_ad_delivery_tally;

ALTER TABLE social_human
    DROP COLUMN ad_delivery_tally;
