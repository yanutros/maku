<script lang="ts">
  // La tasse au cœur de Ko-fi, redessinée en traits simples : la tasse suit la couleur du texte, le
  // cœur garde le corail de Ko-fi.
  let { size = 18 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true">
  <path d="M3 6h12.5A1.5 1.5 0 0 1 17 7.5V13a4 4 0 0 1-4 4H7a4 4 0 0 1-4-4z" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round" />
  <path d="M17 8h1.5a2.5 2.5 0 0 1 0 5H17" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
  <path d="M10 14.6l-.7-.6C7.3 12.2 6.4 11.4 6.4 10.3a2 2 0 0 1 2-2c.6 0 1.2.3 1.6.8.4-.5 1-.8 1.6-.8a2 2 0 0 1 2 2c0 1.1-.9 1.9-2.9 3.7z" fill="#ff5e5b" />
</svg>
