-- Keep the first label of each set.
UPDATE job SET labels = json_extract(labels, '$[0]') WHERE labels IS NOT NULL;
ALTER TABLE job RENAME COLUMN labels TO label;

UPDATE compute_node SET labels = json_extract(labels, '$[0]') WHERE labels IS NOT NULL;
ALTER TABLE compute_node RENAME COLUMN labels TO label;
