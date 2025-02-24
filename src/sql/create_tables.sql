CREATE SCHEMA vsc_cv;

CREATE TABLE vsc_cv.languages(
  id SMALLINT PRIMARY KEY,
  name VARCHAR(20) UNIQUE
);

CREATE TABLE vsc_cv.licenses(
  id SMALLINT PRIMARY KEY,
  name VARCHAR(20) UNIQUE
);

CREATE TABLE vsc_cv.status(
  id SMALLINT PRIMARY KEY,
  name VARCHAR(15) UNIQUE
);

CREATE TABLE vsc_cv.contracts(
  contract_addr VARCHAR(68) PRIMARY KEY,
  bytecode_cid VARCHAR(59) NOT NULL,
  hive_username VARCHAR(16) NOT NULL,
  request_ts TIMESTAMP NOT NULL,
  verified_ts TIMESTAMP,
  status SMALLINT NOT NULL REFERENCES vsc_cv.status(id),
  exports jsonb,
  license SMALLINT REFERENCES vsc_cv.licenses(id),
  lang SMALLINT NOT NULL REFERENCES vsc_cv.languages(id),
  dependencies jsonb
);

CREATE TABLE vsc_cv.source_code(
  contract_addr VARCHAR(68) NOT NULL REFERENCES vsc_cv.contracts(contract_addr),
  fname VARCHAR(50) NOT NULL,
  is_lockfile BOOLEAN NOT NULL DEFAULT FALSE,
  content VARCHAR,
  PRIMARY KEY(contract_addr, fname)
);

INSERT INTO vsc_cv.status(id, name) VALUES (0, 'pending');
INSERT INTO vsc_cv.status(id, name) VALUES (1, 'queued');
INSERT INTO vsc_cv.status(id, name) VALUES (2, 'in progress');
INSERT INTO vsc_cv.status(id, name) VALUES (3, 'success');
INSERT INTO vsc_cv.status(id, name) VALUES (4, 'failed');
INSERT INTO vsc_cv.status(id, name) VALUES (5, 'not match');

-- Names must follow SPDX identifier listed in https://spdx.org/licenses
-- Full text may be found in https://github.com/spdx/license-list-data/tree/main/text
INSERT INTO vsc_cv.licenses(id, name) VALUES (0, 'MIT');
INSERT INTO vsc_cv.licenses(id, name) VALUES (1, 'Apache-2.0');
INSERT INTO vsc_cv.licenses(id, name) VALUES (2, 'GPL-3.0-only');
INSERT INTO vsc_cv.licenses(id, name) VALUES (3, 'GPL-3.0-or-later');
INSERT INTO vsc_cv.licenses(id, name) VALUES (4, 'LGPL-3.0-only');
INSERT INTO vsc_cv.licenses(id, name) VALUES (5, 'LGPL-3.0-or-later');
INSERT INTO vsc_cv.licenses(id, name) VALUES (6, 'AGPL-3.0-only');
INSERT INTO vsc_cv.licenses(id, name) VALUES (7, 'AGPL-3.0-or-later');
INSERT INTO vsc_cv.licenses(id, name) VALUES (8, 'MPL 2.0');
INSERT INTO vsc_cv.licenses(id, name) VALUES (9, 'BSL-1.0');
INSERT INTO vsc_cv.licenses(id, name) VALUES (10, 'WTFPL');
INSERT INTO vsc_cv.licenses(id, name) VALUES (11, 'Unlicense');

INSERT INTO vsc_cv.languages(id, name) VALUES (0, 'assemblyscript');
INSERT INTO vsc_cv.languages(id, name) VALUES (1, 'golang');
INSERT INTO vsc_cv.languages(id, name) VALUES (2, 'rust');