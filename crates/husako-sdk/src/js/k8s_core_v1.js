// "k8s/core/v1" module: Namespace, Service, ConfigMap

import { _ResourceBuilder } from "husako/_base";

export class Namespace extends _ResourceBuilder {
  constructor() {
    super("v1", "Namespace");
  }
}

export class Service extends _ResourceBuilder {
  constructor() {
    super("v1", "Service");
  }
}

export class ConfigMap extends _ResourceBuilder {
  constructor() {
    super("v1", "ConfigMap");
  }
}
