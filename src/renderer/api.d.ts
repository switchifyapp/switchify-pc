export {};

declare global {
  interface Window {
    switchifyPc: {
      appName: string;
      status: string;
    };
  }
}
