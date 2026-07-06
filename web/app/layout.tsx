import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "OxideHV — Hypervisor Fleet Simulator",
  description: "A Rust/WASM simulation of a NUMA-aware VM scheduler, live migration, GPU/NVMe virtualization, and elastic autoscaling.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body className="font-mono antialiased">{children}</body>
    </html>
  );
}
