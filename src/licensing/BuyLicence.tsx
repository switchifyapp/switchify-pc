import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink } from "lucide-react";
import { Button } from "../ui/controls";

// Test checkout only. Replace this URL and the exact opener scope together
// after live fulfilment is approved; v1.4.0 is still held.
export const licencePurchaseUrl = "https://buy.stripe.com/test_dRm28t9gZ2sK3wKezb87K01";

export function BuyLicence() {
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const buy = async () => {
    setBusy(true);
    setFailed(false);
    try { await openUrl(licencePurchaseUrl); }
    catch { setFailed(true); }
    finally { setBusy(false); }
  };
  return <div className="licence-buy">
    <Button className="secondary" disabled={busy} onClick={() => void buy()}>
      Buy licence <ExternalLink size={18} aria-hidden="true" />
    </Button>
    <p className="licence-supporting">Test checkout · no real payment</p>
    {failed && <p className="field-error" role="alert">Could not open checkout in your browser. Try again when your browser is available.</p>}
  </div>;
}
