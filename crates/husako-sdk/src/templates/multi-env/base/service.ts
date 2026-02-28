import { Service } from "k8s/core/v1";
import { name, namespace, label } from "k8s/meta/v1";

export function nginxService(ns: string) {
  return Service()
    .metadata(
      name("nginx").namespace(ns).label("app", "nginx").label("env", ns),
    )
    .selector({ app: "nginx" })
    .ports([{ port: 80, targetPort: 80 }]);
}
