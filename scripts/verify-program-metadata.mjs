import fs from "node:fs";
import { pathToFileURL } from "node:url";

export const UPGRADEABLE_LOADER =
  "BPFLoaderUpgradeab1e11111111111111111111111";

export function verifyProgramMetadata(
  metadata,
  expectedProgramId,
  expectedUpgradeAuthority,
) {
  if (metadata.programId !== expectedProgramId) {
    throw new Error(
      `program id mismatch: expected ${expectedProgramId}, got ${metadata.programId ?? "missing"}`,
    );
  }
  if (metadata.owner !== UPGRADEABLE_LOADER) {
    throw new Error(
      `unexpected loader owner: expected ${UPGRADEABLE_LOADER}, got ${metadata.owner ?? "missing"}`,
    );
  }
  if (
    typeof metadata.programdataAddress !== "string" ||
    metadata.programdataAddress.length === 0
  ) {
    throw new Error("upgradeable program is missing a ProgramData address");
  }

  const authority = metadata.authority ?? null;
  if (expectedUpgradeAuthority === "immutable") {
    if (authority !== null) {
      throw new Error(`program remains upgradeable by ${authority}`);
    }
  } else if (authority !== expectedUpgradeAuthority) {
    throw new Error(
      `upgrade authority mismatch: expected ${expectedUpgradeAuthority}, got ${authority ?? "immutable"}`,
    );
  }
  if (!Number.isSafeInteger(metadata.lastDeploySlot) || metadata.lastDeploySlot < 0) {
    throw new Error("program metadata has an invalid deployment slot");
  }
  if (!Number.isSafeInteger(metadata.dataLen) || metadata.dataLen <= 0) {
    throw new Error("program metadata has an invalid deployed data length");
  }

  return {
    programId: metadata.programId,
    programdataAddress: metadata.programdataAddress,
    authority,
    lastDeploySlot: metadata.lastDeploySlot,
    dataLen: metadata.dataLen,
  };
}

async function main() {
  const [expectedProgramId, expectedUpgradeAuthority] = process.argv.slice(2);
  if (!expectedProgramId || !expectedUpgradeAuthority) {
    throw new Error(
      "usage: verify-program-metadata.mjs <program-id> <upgrade-authority|immutable>",
    );
  }
  const input = fs.readFileSync(0, "utf8");
  const verified = verifyProgramMetadata(
    JSON.parse(input),
    expectedProgramId,
    expectedUpgradeAuthority,
  );
  process.stdout.write(`${JSON.stringify(verified, null, 2)}\n`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`upgrade metadata verification failed: ${error.message}\n`);
    process.exitCode = 1;
  });
}
