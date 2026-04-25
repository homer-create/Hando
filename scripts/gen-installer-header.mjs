// Generates desktop/src-tauri/assets/installer-header.png (150×57)
// Run from repo root: node scripts/gen-installer-header.mjs
import sharp from 'sharp';

const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="150" height="57">
  <rect width="150" height="57" fill="#ffffff"/>

  <!-- App icon scaled to 35×35 at (11, 11) -->
  <g transform="translate(11,11) scale(0.03418)">
    <rect width="1024" height="1024" rx="240" ry="240" fill="#5e00ff"/>
    <path d="M332.15,809.58h-191.42l191.96-503.9c20.93-54.95,73.62-91.26,132.42-91.26h191.42l-191.96,503.9c-20.93,54.95-73.62,91.26-132.42,91.26Z" fill="#ceff04"/>
    <path d="M512,593.83l47.42,124.49c20.93,54.95,73.62,91.26,132.42,91.26h191.42l-47.42-124.49c-20.93-54.95-73.62-91.26-132.42-91.26h-191.42Z" fill="#ceff04"/>
  </g>

  <!-- Hando wordmark -->
  <text x="54" y="35"
    font-family="'Segoe UI',system-ui,sans-serif"
    font-size="20" font-weight="700" fill="#0a0a0a">Hando</text>
  <text x="55" y="50"
    font-family="'Segoe UI',system-ui,sans-serif"
    font-size="10" fill="#71717a">Image Optimizer</text>
</svg>`;

await sharp(Buffer.from(svg))
  .png()
  .toFile('desktop/src-tauri/assets/installer-header.png');

console.log('Generated desktop/src-tauri/assets/installer-header.png (150×57)');
