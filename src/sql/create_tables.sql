CREATE SCHEMA vsc_cv;

CREATE TABLE vsc_cv.languages(
  id SMALLSERIAL PRIMARY KEY,
  name VARCHAR(20)
);

CREATE TABLE vsc_cv.licenses(
  id SMALLSERIAL PRIMARY KEY,
  name VARCHAR(20)
);

CREATE TABLE vsc_cv.status(
  id SMALLSERIAL PRIMARY KEY,
  name VARCHAR(20)
);

CREATE TABLE vsc_cv.contracts(
  contract_addr VARCHAR(68) PRIMARY KEY,
  bytecode_cid VARCHAR(59) NOT NULL,
  hive_username VARCHAR(16) NOT NULL,
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
  content VARCHAR,
  PRIMARY KEY(contract_addr, fname)
);

INSERT INTO vsc_cv.status(name) VALUES ('pending');
INSERT INTO vsc_cv.status(name) VALUES ('success');
INSERT INTO vsc_cv.status(name) VALUES ('failed');

INSERT INTO vsc_cv.licenses(name) VALUES ('MIT');
INSERT INTO vsc_cv.licenses(name) VALUES ('Apache 2.0');
INSERT INTO vsc_cv.licenses(name) VALUES ('GPLv3');
INSERT INTO vsc_cv.licenses(name) VALUES ('LGPLv3');
INSERT INTO vsc_cv.licenses(name) VALUES ('AGPLv3');
INSERT INTO vsc_cv.licenses(name) VALUES ('MPL 2.0');
INSERT INTO vsc_cv.licenses(name) VALUES ('WTFPL');
INSERT INTO vsc_cv.licenses(name) VALUES ('Unlicense');

INSERT INTO vsc_cv.languages(name) VALUES ('assemblyscript');
INSERT INTO vsc_cv.languages(name) VALUES ('golang');
INSERT INTO vsc_cv.languages(name) VALUES ('rust');