import { metadata } from "husako";

export function appMetadata(appName: string) {
  return metadata().name(appName).label("app", appName);
}
