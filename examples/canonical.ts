import * as husako from "husako";
import { Deployment } from "k8s/apps/v1";
import { name, namespace, label, cpu, memory, requests, limits } from "husako";

const nginx_metadata = name("nginx")
  .namespace("nginx-ns")
  .label("key1", "value1")
  .label("key2", "value2");

const another_labels_1 = label("key3", "value3").label("key4", "value4");
const another_labels_2 = label("key5", "value5").label("key6", "value6");

const nginx = new Deployment()
  .metadata(husako.merge([nginx_metadata, another_labels_1, another_labels_2]))
  .replicas(3)
  .selector({ matchLabels: { app: "nginx" } })
  .template({ metadata: { labels: { app: "nginx" } } })
  .containers([{
    name: "nginx",
    image: "nginx:1.25",
    resources: {
      requests: { cpu: "1", memory: "2Gi" },
      limits: { cpu: "500m", memory: "1Gi" },
    },
  }]);

husako.build([nginx]);
