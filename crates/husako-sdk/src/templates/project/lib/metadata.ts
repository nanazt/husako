import { name, label } from "husako";

export function metadata(appName: string) {
  return name(appName).label("app", appName);
}
