/// <reference types="vite/client" />
// `import.meta.glob` (locale-literals.test.ts kaynak taraması) buradan gelir;
// vite zaten devDependency, yeni paket eklenmedi.

// Vite `define` ile enjekte edilen build-time sabitleri.
declare const __APP_VERSION__: string;
