import { service } from "k8s/core/v1";
import { metadata } from "husako";

export function nginxService(ns: string) {
  return service()
    .metadata(
      metadata().name("nginx").namespace(ns).label("app", "nginx").label("env", ns)
    )
    .selector({ app: "nginx" })
    .ports([{ port: 80, targetPort: 80 }]);
}
