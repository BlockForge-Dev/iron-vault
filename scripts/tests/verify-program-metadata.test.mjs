import assert from "node:assert/strict";
import test from "node:test";

import {
  UPGRADEABLE_LOADER,
  verifyProgramMetadata,
} from "../verify-program-metadata.mjs";

const programId = "2UWmTuefm4gqbfuZP36NSJMMSKLM4Rbop25jf1uBZAu1";
const multisig = "Bo8iHtbsaLxRWrb39sZipzrNywzbxFXjXQQBYZXsKJc1";
const baseMetadata = Object.freeze({
  programId,
  owner: UPGRADEABLE_LOADER,
  programdataAddress: "9rY8Xfb87vC9CSbN9SdtwR4UkmKUXfVbUr8WzCrKjiVK",
  authority: multisig,
  lastDeploySlot: 123,
  dataLen: 456,
});

test("accepts the exact expected multisig authority", () => {
  assert.deepEqual(verifyProgramMetadata(baseMetadata, programId, multisig), {
    programId,
    programdataAddress: baseMetadata.programdataAddress,
    authority: multisig,
    lastDeploySlot: 123,
    dataLen: 456,
  });
});

test("rejects a developer key when a multisig is required", () => {
  assert.throws(
    () =>
      verifyProgramMetadata(
        { ...baseMetadata, authority: "DeveloperLaptop111111111111111111111111111" },
        programId,
        multisig,
      ),
    /upgrade authority mismatch/,
  );
});

test("distinguishes immutable programs from upgradeable programs", () => {
  assert.throws(
    () => verifyProgramMetadata({ ...baseMetadata, authority: null }, programId, multisig),
    /got immutable/,
  );
  assert.equal(
    verifyProgramMetadata({ ...baseMetadata, authority: null }, programId, "immutable")
      .authority,
    null,
  );
});

test("rejects an unexpected loader or missing ProgramData", () => {
  assert.throws(
    () => verifyProgramMetadata({ ...baseMetadata, owner: "WrongLoader" }, programId, multisig),
    /unexpected loader owner/,
  );
  assert.throws(
    () =>
      verifyProgramMetadata(
        { ...baseMetadata, programdataAddress: null },
        programId,
        multisig,
      ),
    /missing a ProgramData address/,
  );
});

test("rejects mismatched identity and malformed deployment facts", () => {
  assert.throws(
    () => verifyProgramMetadata(baseMetadata, "WrongProgram", multisig),
    /program id mismatch/,
  );
  assert.throws(
    () => verifyProgramMetadata({ ...baseMetadata, dataLen: 0 }, programId, multisig),
    /invalid deployed data length/,
  );
});
