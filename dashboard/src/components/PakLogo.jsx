export default function PakLogo({ size = 22 }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 100 100"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      style={{ filter: "drop-shadow(0 0 4px rgba(57,255,20,0.4))" }}
    >
      <defs>
        <filter id="logo-glow" x="-20%" y="-20%" width="140%" height="140%">
          <feGaussianBlur stdDeviation="3" result="blur" />
          <feComposite in="SourceGraphic" in2="blur" operator="over" />
        </filter>
      </defs>
      {/* Background */}
      <rect width="100" height="100" rx="20" fill="#050a05" />
      {/* Hexagon */}
      <polygon
        points="50,15 80,32 80,68 50,85 20,68 20,32"
        fill="none"
        stroke="#1a2a1a"
        strokeWidth="4"
      />
      {/* Prompt */}
      <polyline
        points="32,35 55,50 32,65"
        fill="none"
        stroke="#39ff14"
        strokeWidth="8"
        strokeLinecap="round"
        strokeLinejoin="round"
        filter="url(#logo-glow)"
      />
      {/* Cursor */}
      <rect
        x="62"
        y="56"
        width="12"
        height="8"
        rx="2"
        fill="#00ffff"
        filter="url(#logo-glow)"
      />
    </svg>
  );
}