import { Service } from "k8s/core/v1";
import { name, namespace, label } from "husako";

export function nginxService(ns: string) {
  return new Service()
    .metadata(
      name("nginx").namespace(ns).label("app", "nginx").label("env", ns)
    )
    .spec({
      selector: { app: "nginx" },
      ports: [{ port: 80, targetPort: 80 }],
    });
}
