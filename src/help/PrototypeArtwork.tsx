// SVG artwork adapted from the local switch-interface-animation prototypes.
import { useId } from "react";
import type { DemonstrationKind, DemoPlatform } from "./demonstrations";

export function PrototypeArtwork({ kind, step, platform }: { kind: DemonstrationKind; step: number; platform: DemoPlatform }) {
  const prefix = useId();
  if (kind === "jack") return (
<svg className="prototype-artwork" viewBox="0 0 720 405" aria-hidden="true" focusable="false">
          <defs>
            <linearGradient id={`${prefix}-j-body`} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="#efedf4" /><stop offset="100%" stopColor="#d5d2dc" />
            </linearGradient>
            <linearGradient id={`${prefix}-j-metal`} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="#f4f6f9" /><stop offset="45%" stopColor="#c5cad3" /><stop offset="100%" stopColor="#8a909b" />
            </linearGradient>
            <linearGradient id={`${prefix}-j-barrel`} x1="0" y1="0" x2="1" y2="0">
              <stop offset="0%" stopColor="#2a2a30" /><stop offset="100%" stopColor="#1c1c22" />
            </linearGradient>
            <linearGradient id={`${prefix}-j-boot`} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="#3a3a42" /><stop offset="100%" stopColor="#121216" />
            </linearGradient>
            <clipPath id={`${prefix}-j-clip`}><rect x="-120" y="0" width="560" height="405" /></clipPath>
          </defs>
          <rect width="720" height="405" fill="#f7f6fa" />
          <ellipse cx="400" cy="358" rx="220" ry="15" fill="#e4e2ea" />
          <rect x="448" y="72" width="200" height="270" rx="14" fill={`url(#${prefix}-j-body)`} stroke="#a9a5b2" strokeWidth="1.5" />
          <rect x="448" y="72" width="12" height="270" fill="#c4c0cc" />
          <rect x="470" y="88" width="164" height="28" rx="6" fill="#d32f2f" />
          <text x="552" y="106" textAnchor="middle" fill="#fff" fontSize="11" fontWeight="700" letterSpacing="0.06em">SWITCH INTERFACE</text>
          <g fontSize="11" fontWeight="650" fill="#5c5764">
            <g transform="translate(0,148)"><rect x="448" y="-14" width="64" height="28" rx="3" fill={`url(#${prefix}-j-barrel)`} /><ellipse cx="448" cy="0" rx="8" ry="14" fill="#0a0a0c" stroke="#8e8a96" strokeWidth="2" /><ellipse className="glow" cx="448" cy="0" rx="12" ry="18" fill="none" stroke="#d32f2f" strokeWidth="2.5" opacity={step > 0 ? 1 : 0} /><ellipse cx="448" cy="0" rx="3.5" ry="6.5" fill="#000" /><text x="528" y="4">1</text></g>
            <g transform="translate(0,202)"><rect x="448" y="-14" width="64" height="28" rx="3" fill={`url(#${prefix}-j-barrel)`} /><ellipse cx="448" cy="0" rx="8" ry="14" fill="#0a0a0c" stroke="#8e8a96" strokeWidth="2" /><ellipse cx="448" cy="0" rx="3.5" ry="6.5" fill="#000" /><text x="528" y="4">2</text></g>
            <g transform="translate(0,256)"><rect x="448" y="-14" width="64" height="28" rx="3" fill={`url(#${prefix}-j-barrel)`} /><ellipse cx="448" cy="0" rx="8" ry="14" fill="#0a0a0c" stroke="#8e8a96" strokeWidth="2" /><ellipse cx="448" cy="0" rx="3.5" ry="6.5" fill="#000" /><text x="528" y="4">3</text></g>
            <g transform="translate(0,310)"><rect x="448" y="-14" width="64" height="28" rx="3" fill={`url(#${prefix}-j-barrel)`} /><ellipse cx="448" cy="0" rx="8" ry="14" fill="#0a0a0c" stroke="#8e8a96" strokeWidth="2" /><ellipse cx="448" cy="0" rx="3.5" ry="6.5" fill="#000" /><text x="528" y="4">4</text></g>
          </g>
          <g clipPath={`url(#${prefix}-j-clip)`}><g transform={`translate(${step > 0 ? 240 : 80} 0)`}><g transform="translate(0,148)">
            <path d="M -40 0 H 152" fill="none" stroke="#1c1b1f" strokeWidth="13" strokeLinecap="round" />
            <rect x="148" y="-16" width="52" height="32" rx="10" fill={`url(#${prefix}-j-boot)`} />
            <rect x="154" y="-11" width="40" height="22" rx="7" fill="#2a2a32" />
            <rect x="196" y="-8" width="18" height="16" rx="1.5" fill={`url(#${prefix}-j-metal)`} />
            <rect x="214" y="-8" width="4" height="16" fill="#0e0e12" />
            <rect x="218" y="-8" width="12" height="16" fill={`url(#${prefix}-j-metal)`} />
            <rect x="230" y="-8" width="4" height="16" fill="#0e0e12" />
            <rect x="234" y="-6" width="14" height="12" fill={`url(#${prefix}-j-metal)`} />
            <path d="M248 -6 L258 0 L248 6 Z" fill={`url(#${prefix}-j-metal)`} />
          </g></g></g>
          <g pointerEvents="none">
            <ellipse cx="448" cy="148" rx="8" ry="14" fill="#0a0a0c" stroke="#8e8a96" strokeWidth="2" />
            <ellipse className="glow" cx="448" cy="148" rx="13" ry="19" fill="none" stroke="#d32f2f" strokeWidth="2.5" opacity={step > 0 ? 1 : 0} />
            <ellipse cx="448" cy="148" rx="3.5" ry="6.5" fill="#000" />
            <ellipse cx="448" cy="202" rx="8" ry="14" fill="#0a0a0c" stroke="#8e8a96" strokeWidth="2" /><ellipse cx="448" cy="202" rx="3.5" ry="6.5" fill="#000" />
            <ellipse cx="448" cy="256" rx="8" ry="14" fill="#0a0a0c" stroke="#8e8a96" strokeWidth="2" /><ellipse cx="448" cy="256" rx="3.5" ry="6.5" fill="#000" />
            <ellipse cx="448" cy="310" rx="8" ry="14" fill="#0a0a0c" stroke="#8e8a96" strokeWidth="2" /><ellipse cx="448" cy="310" rx="3.5" ry="6.5" fill="#000" />
          </g>
        </svg>
  );
  if (kind === "interface") {
    const knobX = step === 0 ? 292 : 428;
    return (
<svg className="prototype-artwork" viewBox="0 0 720 405" aria-hidden="true" focusable="false">
          <rect width="720" height="405" fill="#f7f6fa" />
          <ellipse cx="360" cy="352" rx="240" ry="14" fill="#e4e2ea" />

          <rect x="180" y="86" width="360" height="236" rx="14" fill="#efedf4" stroke="#a9a5b2" strokeWidth="1.5" />
          <rect x="202" y="102" width="316" height="28" rx="6" fill="#d32f2f" />
          <text x="360" y="120" textAnchor="middle" fill="#fff" fontSize="11" fontWeight="700" letterSpacing="0.06em">SWITCH INTERFACE</text>
          <text x="360" y="158" textAnchor="middle" fontSize="12" fontWeight="650" fill="#5c5764">MODE</text>

          <rect x="264" y="170" width="192" height="44" rx="22" fill="#d5d2dc" stroke="#b9b5c2" />
          <circle className="mode-knob" cx={knobX} cy="192" r="18" fill="#d32f2f" />

          <g stroke="#3a3a42" strokeWidth="2" fill="none" strokeLinecap="round">
            <g transform="translate(292,192)"><rect x="-6" y="-11" width="12" height="22" rx="6" /><path d="M0 -11 v6" /></g>
            <g transform="translate(360,192)"><rect x="-13" y="-7" width="26" height="14" rx="6" /><circle cx="-6" cy="0" r="1.6" fill="#3a3a42" /><circle cx="6" cy="-2" r="1.6" fill="#3a3a42" /><circle cx="9" cy="2" r="1.6" fill="#3a3a42" /></g>
            <g transform="translate(428,192)"><rect x="-14" y="-8" width="28" height="16" rx="3" /><path d="M-9 -3 h3 M-3 -3 h3 M3 -3 h3 M-9 3 h18" /></g>
          </g>
          <g fontSize="11" fill="#5c5764" textAnchor="middle">
            <text x="292" y="240">Mouse</text>
            <text x="360" y="240">Gamepad</text>
            <text x="428" y="240" fontWeight={step > 0 ? 700 : 400} fill={step > 0 ? "#1c1b1f" : "#5c5764"}>Keyboard</text>
          </g>
          <g transform="translate(0,278)"><rect x="522" y="-14" width="18" height="28" rx="3" fill="#2a2a30" /><ellipse cx="540" cy="0" rx="8" ry="14" fill="#0a0a0c" stroke="#8e8a96" strokeWidth="2" /></g>

          <path d="M548 278 H 600" fill="none" stroke="#1c1b1f" strokeWidth="8" strokeLinecap="round" />
          <g transform="translate(640,278)">
            <circle r="42" fill="#2a2a32" />
            <circle r="31" fill="#1a1a20" />
            <circle className="plunger" r="22" fill="#d32f2f" cy={step === 2 ? 5 : -2} />
          </g>

          <g className="mode-badge" opacity={step === 0 ? 1 : 0} transform="translate(360,300)">
            <rect x="-70" y="-15" width="140" height="30" rx="6" fill="#ffdad6" stroke="#ba1a1a" />
            <text textAnchor="middle" y="5" fontSize="12" fontWeight="700" fill="#ba1a1a">Nothing learned</text>
          </g>
          <g className="key-badge-art" opacity={step >= 2 ? 1 : 0} transform="translate(360,300)">
            <rect x="-40" y="-16" width="80" height="32" rx="6" fill="#f3f2f6" stroke="#e2e0e8" strokeWidth="2" />
            <text textAnchor="middle" y="5" fontSize="14" fontWeight="700" fontFamily="ui-monospace,Consolas,monospace" fill="#1c1b1f">{step === 3 ? "F13" : "Space"}</text>
          </g>
          <g opacity={step === 3 ? 1 : 0}><text x="360" y="382" textAnchor="middle" fontSize="12" fill="#49454f">Programmed so it never types</text></g>
        </svg>
    );
  }
  if (kind === "usb") return (
<svg className="prototype-artwork" viewBox="0 0 720 405" aria-hidden="true" focusable="false">
          <rect width="720" height="405" fill="#f7f6fa" />

          <rect x="420" y="100" width="220" height="140" rx="10" fill="#d8d6de" stroke="#a9a5b2" />
          <rect x="440" y="118" width="180" height="100" rx="4" fill="#1e232e" />
          <rect x="500" y="250" width="60" height="14" rx="2" fill="#b8b5c0" />
          <rect className="usb-port" x="455" y="248" width="28" height="10" rx="2" fill="#1c1b1f" />

          <g transform={`translate(${step > 0 ? 0 : -120} 0)`}>
            <path d="M80 255 H 430" fill="none" stroke="#1c1b1f" strokeWidth="8" strokeLinecap="round" />
            <rect x="40" y="220" width="70" height="70" rx="10" fill="#2a2a32" />
            <circle cx="75" cy="255" r="18" fill="#d32f2f" />
            <rect x="420" y="246" width="36" height="16" rx="2" fill="#c5cad3" />
            <rect x="448" y="249" width="14" height="10" rx="1" fill="#8a909b" />
          </g>

          <g className="usb-badge" opacity={step > 0 ? 1 : 0}>
            <rect x="300" y="355" width="120" height="28" rx="6" fill="#e8f5e9" stroke="#2e7d32" />
            <text x="360" y="374" textAnchor="middle" fontSize="12" fontWeight="700" fill="#2e7d32">Connected · Learn key</text>
          </g>
        </svg>
  );
  if (kind === "learn") return (
<svg className="prototype-artwork" viewBox="0 0 720 405" aria-hidden="true" focusable="false">
          <rect width="720" height="405" fill="#f7f6fa" />

          <g transform="translate(200,70)">
            <rect className="learn-dialog" width="320" height="200" rx="12" fill="#fff" stroke="#efb8b4" strokeWidth="2" />
            <circle cx="160" cy="56" r="22" fill="none" stroke="#d32f2f" strokeWidth="2.5" />
            <path d="M148 56 h24 M160 44 v24" stroke="#d32f2f" strokeWidth="2.5" strokeLinecap="round" />
            <text x="160" y="100" textAnchor="middle" fontSize="16" fontWeight="700" fill="#1c1b1f">Learning your switch</text>
            <text x="160" y="124" textAnchor="middle" fontSize="12" fill="#49454f">Press your switch once</text>
            <rect x="110" y="150" width="100" height="32" rx="6" fill="#fff" stroke="#e2e0e8" />
            <text x="160" y="170" textAnchor="middle" fontSize="12" fill="#1c1b1f">Cancel capture</text>
          </g>

          <g data-anim="learn-switch" transform="translate(560,250)">
            <circle r="48" fill="#2a2a32" />
            <circle r="36" fill="#1a1a20" />
            <circle className="plunger" r="26" fill="#d32f2f" cy={step === 1 ? 5 : -2} />
          </g>

          <g className="learn-badge" opacity={step === 2 ? 1 : 0} transform="translate(360,300)">
            <rect x="-36" y="-16" width="72" height="32" rx="6" fill="#f3f2f6" stroke="#e2e0e8" strokeWidth="2" />
            <text textAnchor="middle" y="5" fontSize="14" fontWeight="700" fontFamily="ui-monospace,Consolas,monospace">F13</text>
          </g>

        </svg>
  );
  if (kind === "select") return (
<svg className="prototype-artwork" viewBox="0 0 720 405" aria-hidden="true" focusable="false">
          <rect width="720" height="405" fill="#f7f6fa" />
          <rect x="140" y="48" width="440" height="300" rx="10" fill="#fff" stroke="#e2e0e8" />
          <text x="164" y="84" fontSize="14" fontWeight="700">Head switch</text>
          <text x="164" y="108" fontSize="12" fill="#49454f">Key</text>
          <rect x="164" y="118" width="56" height="28" rx="5" fill="#f3f2f6" stroke="#e2e0e8" strokeWidth="2" />
          <text x="192" y="137" textAnchor="middle" fontSize="13" fontWeight="700" fontFamily="ui-monospace,Consolas,monospace">F13</text>
          <text x="164" y="178" fontSize="12" fill="#49454f">Press and release</text>
          <g className="select-menu">
            <rect x="164" y="188" width="240" height="140" rx="6" fill="#fff" stroke="#1f6fa2" strokeWidth="2" />
            <g fontSize="13">
              <rect className="opt" data-i="0" x="168" y="194" width="232" height="30" rx="4" fill="transparent" />
              <text x="180" y="214">Next</text>
              <rect className="opt" data-i="1" x="168" y="226" width="232" height="30" rx="4" fill="transparent" />
              <text x="180" y="246">Previous</text>
              <rect className="opt" data-i="2" opacity={step > 0 ? 1 : 0} x="168" y="258" width="232" height="30" rx="4" fill="#fff1f1" />
              <text x="180" y="278" fontWeight="700" fill="#410002">Select</text>
              <rect className="opt" data-i="3" x="168" y="290" width="232" height="30" rx="4" fill="transparent" />
              <text x="180" y="310">Stop scanning</text>
            </g>
          </g>

        </svg>
  );
  if (kind === "access") return (
<svg className="prototype-artwork" viewBox="0 0 720 405" aria-hidden="true" focusable="false">
          <rect width="720" height="405" fill="#e8e6ee" />
          <rect x="60" y="40" width="600" height="320" rx="12" fill="#fff" stroke="#c8c5cf" />
          <rect x="60" y="40" width="180" height="320" rx="12" fill="#f3f2f6" />
          <rect x="60" y="40" width="180" height="44" fill="#f3f2f6" />
          <text x="80" y="68" fontSize="13" fontWeight="700">System Settings</text>
          <g fontSize="12" fill="#49454f">
            <text x="80" y="110">Wi‑Fi</text>
            <text x="80" y="140">Bluetooth</text>
            <rect className="access-row" x="70" y="158" width="160" height="28" rx="6" fill="#d32f2f" />
            <text x="80" y="176" fill="#fff" fontWeight="650">Privacy &amp; Security</text>
            <text x="80" y="220">Keyboard</text>
          </g>
          <text x="270" y="78" fontSize="16" fontWeight="700">Accessibility</text>
          <g className="access-list" fontSize="13">
            <rect x="270" y="110" width="360" height="44" rx="8" fill="#f7f6fa" stroke="#e2e0e8" />
            <text x="290" y="137">Terminal</text>
            <rect className="access-toggle" x="580" y="120" width="36" height="22" rx="11" fill="#c8c5cf" />
            <circle className="access-knob" cx="591" cy="131" r="8" fill="#fff" />

            <rect className="access-target" x="270" y="166" width="360" height="44" rx="8" fill="#fff1f1" stroke="#d32f2f" strokeWidth="2" />
            <text x="290" y="193" fontWeight="700">Switchify PC</text>
            <rect className="access-toggle-b" style={{fill: step === 2 ? "#2e7d32" : "#c8c5cf"}} x="580" y="176" width="36" height="22" rx="11" fill="#c8c5cf" />
            <circle className="access-knob-b" cx={step === 2 ? 605 : 591} cy="187" r="8" fill="#fff" />
          </g>

        </svg>
  );
  if (kind === "pair") return (
<svg className="prototype-artwork" viewBox="0 0 720 405" aria-hidden="true" focusable="false">
          <rect width="720" height="405" fill="#f7f6fa" />

          <g transform="translate(80,50)">
            <rect width="160" height="300" rx="24" fill="#1c1b1f" />
            <rect x="10" y="28" width="140" height="244" rx="8" fill="#fff" />
            <text x="80" y="60" textAnchor="middle" fontSize="11" fontWeight="700" fill="#d32f2f">Switchify</text>
            <text x="80" y="100" textAnchor="middle" fontSize="12" fill="#49454f">Pairing code</text>
            <text className="pair-code-phone" x="80" y="150" textAnchor="middle" fontSize="28" fontWeight="700" letterSpacing="0.12em">482163</text>
          </g>

          <g transform="translate(320,80)">
            <rect width="340" height="220" rx="10" fill="#fff" stroke="#e2e0e8" />
            <text x="24" y="40" fontSize="16" fontWeight="700">Pairing requests</text>
            <text x="24" y="64" fontSize="12" fill="#49454f">Compare with your mobile device.</text>
            <rect x="24" y="90" width="292" height="70" rx="8" fill="#f7f6fa" stroke="#e2e0e8" />
            <text className="pair-code-pc" x="170" y="135" textAnchor="middle" fontSize="26" fontWeight="700" letterSpacing="0.12em">482163</text>
            <rect className="pair-approve" opacity={step >= 2 ? 1 : 0.4} x="24" y="180" width="130" height="28" rx="6" fill="#d32f2f" />
            <text x="89" y="198" textAnchor="middle" fill="#fff" fontSize="12" fontWeight="650">Approve</text>
            <rect x="170" y="180" width="130" height="28" rx="6" fill="#fff" stroke="#e2e0e8" />
            <text x="235" y="198" textAnchor="middle" fontSize="12">Reject</text>
          </g>

        </svg>
  );
  if (kind === "startup") return (
<svg className="prototype-artwork" viewBox="0 0 720 405" aria-hidden="true" focusable="false">
          <rect width="720" height="405" fill="#f7f6fa" />
          <g transform="translate(40,60)">
            <text fontSize="13" fontWeight="700" fill="#49454f">START WITH SYSTEM</text>
            <rect x="0" y="30" width="300" height="200" rx="10" fill="#1e232e" />
            <rect className="startup-a" x="20" y="50" width="260" height="140" rx="4" fill="#2a3140" opacity="0.3" />
            <text className="startup-a-label" x="150" y="125" textAnchor="middle" fill="#fff" fontSize="12" opacity="0.5">Desktop</text>
  
            <rect x="0" y={platform === "macos" ? 30 : 200} width="300" height="30" fill="#141820" />
            <circle className="startup-tray" cx="270" cy={platform === "macos" ? 40 : 215} r="8" fill="#d32f2f" opacity={step > 0 ? 1 : 0} />
  
          </g>
          <g transform="translate(380,60)">
            <text fontSize="13" fontWeight="700" fill="#49454f">START MANUALLY</text>
            <rect x="0" y="30" width="300" height="200" rx="10" fill="#e8e6ee" stroke="#c8c5cf" />
            <rect x="40" y="70" width="64" height="64" rx="14" fill="#d32f2f" />
            <text x="72" y="108" textAnchor="middle" fill="#fff" fontSize="20" fontWeight="700">S</text>
            <text x="72" y="160" textAnchor="middle" fontSize="11">Switchify</text>
            <circle className="startup-cursor" cx="90" cy="100" r="6" fill="#fff" stroke="#1c1b1f" strokeWidth="2" opacity={step === 2 ? 1 : 0} />
  
          </g>
        </svg>
  );
  return null;
}
