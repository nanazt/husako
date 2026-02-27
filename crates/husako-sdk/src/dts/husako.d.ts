/** husako: build Kubernetes resources from TypeScript. */
declare const husako: {
  /** Render and emit one or more builder instances as Kubernetes YAML.
   *  Must be called exactly once per entrypoint. */
  build(resources: { _render(): any } | Array<{ _render(): any }>): void;
};

export default husako;
