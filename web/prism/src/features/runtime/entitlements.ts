import type { ProviderAccountEntitlement } from "./model";

// Vocabulary validation only. Account tiers never determine model visibility.
const TIERS: Readonly<Record<string, readonly string[]>> = {
  grok_build: ["unknown", "free", "supergrok", "heavy"],
  grok_web: ["unknown", "basic", "super", "heavy"],
  chatgpt: ["unknown", "free", "go", "plus", "pro5x", "pro20x"],
  claude: ["unknown", "free", "pro", "max5x", "max20x"],
};
const CONFIDENCE: Readonly<Record<string, string>> = {
  provider_subscription: "authoritative", signed_token: "derived", imported_metadata: "declared",
};

export function isKnownEntitlement(value: ProviderAccountEntitlement): boolean {
  return TIERS[value.domain]?.includes(value.tier) === true
    && CONFIDENCE[value.source] === value.confidence;
}
