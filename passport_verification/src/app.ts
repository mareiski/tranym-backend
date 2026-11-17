import express, {
  type Request,
  type Response,
  type NextFunction,
} from "express";
import { ProofResult, QueryResult, ZKPassport } from "@zkpassport/sdk";
import { timingSafeEqual } from "node:crypto";
import "dotenv/config";

const PORT = Number(process.env.PORT ?? 3100);
const HOST = process.env.ZKPASSPORT_HOST ?? "127.0.0.1";
const DOMAIN = process.env.ZKPASSPORT_DOMAIN; // e.g. "yourdomain.com" (the domain the frontend uses)
const INTERNAL_SECRET = process.env.ZKPASSPORT_INTERNAL_SECRET;
const IS_DEV = process.env.NODE_ENV !== "production";

if (!DOMAIN) throw new Error("ZKPASSPORT_DOMAIN is not configured");
if (!INTERNAL_SECRET || INTERNAL_SECRET.length < 32) {
  throw new Error(
    "ZKPASSPORT_INTERNAL_SECRET must contain at least 32 characters",
  );
}

const app = express();
app.use(express.json({ limit: "10mb" })); // proofs are large; no CORS needed, only Lemmy calls this

const zkPassport = new ZKPassport(DOMAIN);

type VerificationRequest = {
  proofs: ProofResult[];
  queryResult: QueryResult;
  expectedNonce: string;
};

type VerificationResponse = {
  verified: boolean;
  userId: string | null;
};

const FAILED: VerificationResponse = { verified: false, userId: null };

// --- auth: only Lemmy may call this ---
function requireInternalSecret(
  req: Request,
  res: Response,
  next: NextFunction,
) {
  const given = Buffer.from(String(req.header("x-internal-secret") ?? ""));
  const expected = Buffer.from(INTERNAL_SECRET!);
  if (given.length !== expected.length || !timingSafeEqual(given, expected)) {
    res.status(403).json({ error: "forbidden" });
    return;
  }
  next();
}

async function verifyProofs({
  proofs,
  queryResult,
  expectedNonce,
}: VerificationRequest): Promise<VerificationResponse> {
  console.log("verifying proofs for nonce", expectedNonce);

  // IMPORTANT: this must be the SAME query the frontend requested (same disclosures),
  // plus the bound nonce.
  const { query } = zkPassport
    .createQuery()
    .bind("custom_data", expectedNonce)
    .done();

  const { verified, uniqueIdentifier } = await zkPassport.verify({
    proofs,
    originalQuery: query,
    queryResult,
    scope: "personhood", // must match the frontend scope
    devMode: IS_DEV,
  });

  if (!verified || !uniqueIdentifier) return FAILED;

  // Belt and braces: also check the bound value in the result directly
  const bound = (queryResult as any)?.bind?.custom_data;
  if (typeof bound !== "string" || bound !== expectedNonce) return FAILED;

  return { verified: true, userId: uniqueIdentifier };
}

app.get("/health", (_req, res) => {
  res.json({ status: "ok" });
});

app.post("/verify", requireInternalSecret, async (req, res, next) => {
  try {
    const body = req.body as Partial<VerificationRequest>;

    if (
      !Array.isArray(body.proofs) ||
      body.proofs.length === 0 ||
      !body.queryResult ||
      typeof body.expectedNonce !== "string" ||
      body.expectedNonce.length === 0 ||
      body.expectedNonce.length > 256
    ) {
      res.status(400).json({ error: "Invalid request body" });
      return;
    }

    const result = await verifyProofs(body as VerificationRequest);
    res.status(200).json(result); // always 200, Lemmy checks `verified`
  } catch (error) {
    // verification errors (bad proof format etc.) count as "not verified"
    console.error("verify failed:", (error as Error).message);
    res.status(200).json(FAILED);
  }
});

app.use((error: Error, _req: Request, res: Response, _next: NextFunction) => {
  console.error("request failed:", error.message);
  if (!res.headersSent)
    res.status(500).json({ error: "Internal server error" });
});

const server = app.listen(PORT, HOST, () => {
  console.log(`zkpassport verifier listening on http://${HOST}:${PORT}`);
});

const shutdown = () => server.close(() => process.exit(0));
process.once("SIGINT", shutdown);
process.once("SIGTERM", shutdown);
