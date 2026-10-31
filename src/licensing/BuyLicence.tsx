import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink } from "lucide-react";
import { Button } from "../ui/controls";

// Keep this URL and the exact Tauri opener scope in sync.
export const licencePurchaseUrl = "https://buy.stripe.com/9B63cx8cV8R8c3gfDf87K00";

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
    {failed && <p className="field-error" role="alert">Could not open checkout in your browser. Try again when your browser is available.</p>}
  </div>;
}
