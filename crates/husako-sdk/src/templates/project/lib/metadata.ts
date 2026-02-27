import { name } from "k8s/meta/v1";

export function appMetadata(appName: string) {
  return name(appName).label("app", appName);
}
