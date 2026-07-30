import type { NextConfig } from "next";
import path from "path";

const nextConfig: NextConfig = {
  // Pins the workspace root to this app, so Next doesn't try to infer it from an unrelated
  // lockfile elsewhere on the machine.
  turbopack: {
    root: path.join(__dirname),
  },
};

export default nextConfig;
