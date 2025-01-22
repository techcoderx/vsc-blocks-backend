-- All validations should be done on the Actix API returning the appropriate response codes.
-- Exceptions raised here are considered assertion errors that should not happen in production.

-- New contract verification
CREATE OR REPLACE FUNCTION vsc_cv.verify_new(
  _contract_addr VARCHAR,
  _bc_cid VARCHAR,
  _username VARCHAR,
  _status SMALLINT,
  _license VARCHAR,
  _lang VARCHAR,
  _dependencies jsonb
)
RETURNS void AS $$
DECLARE
  _licence_id SMALLINT;
  _lang_id SMALLINT;
BEGIN
  SELECT id INTO _licence_id FROM vsc_cv.licenses l WHERE l.name = _license;
  SELECT id INTO _lang_id FROM vsc_cv.languages l WHERE l.name = _lang;
  IF _licence_id IS NULL THEN
    RAISE EXCEPTION 'License % is currently unsupported', _license;
  ELSIF _lang_id IS NULL THEN
    RAISE EXCEPTION 'Language % is currently unsupported', _lang;
  END IF;

  INSERT INTO vsc_cv.contracts(contract_addr, bytecode_cid, hive_username, status, license, lang, dependencies)
    VALUES(_contract_addr, _bc_cid, _username, _status, _licence_id, _lang_id, _dependencies);
END $$
LANGUAGE plpgsql VOLATILE;