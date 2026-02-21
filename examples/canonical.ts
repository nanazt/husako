import { deployment } from "k8s/apps/v1";
import { container } from "k8s/core/v1";
import { selector } from "k8s/_common";
import { metadata, label, cpu, memory, requests, limits, merge, build } from "husako";

const nginx_metadata = metadata()
  .name("nginx")
  .namespace("nginx-ns")
  .label("key1", "value1")
  .label("key2", "value2");

const another_labels_1 = label("key3", "value3").label("key4", "value4");
const another_labels_2 = label("key5", "value5").label("key6", "value6");

const nginx = deployment()
  .metadata(merge([nginx_metadata, another_labels_1, another_labels_2]))
  .replicas(3)
  .selector(selector().matchLabels({ app: "nginx" }))
  .containers([
    container()
      .name("nginx")
      .image("nginx:1.25")
      .resources(
        requests(cpu("250m").memory("128Mi"))
          .limits(cpu("500m").memory("256Mi"))
      )
  ]);

build([nginx]);
