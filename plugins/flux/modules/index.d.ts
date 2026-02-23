import { _ResourceBuilder } from "husako/_base";
import {
  SourceRef,
  GitRepositoryBuilder,
  HelmRepositoryBuilder,
  OCIRepositoryBuilder,
} from "flux/source";

export { GitRepository, HelmRepository, OCIRepository } from "flux/source";
export type { SourceRef, GitRef, OCIRef, GitRepositoryBuilder, HelmRepositoryBuilder, OCIRepositoryBuilder } from "flux/source";

export interface HelmReleaseBuilder extends _ResourceBuilder {
  chart(name: string, version: string | number): this;
  sourceRef(ref: SourceRef | GitRepositoryBuilder | HelmRepositoryBuilder | OCIRepositoryBuilder): this;
  interval(interval: string): this;
  values(values: Record<string, unknown>): this;
  valuesFrom(sources: Array<{ kind: string; name: string; valuesKey?: string }>): this;
  dependsOn(deps: Array<{ name: string; namespace?: string }>): this;
}

export interface KustomizationBuilder extends _ResourceBuilder {
  sourceRef(ref: SourceRef | GitRepositoryBuilder | HelmRepositoryBuilder | OCIRepositoryBuilder): this;
  path(path: string): this;
  interval(interval: string): this;
  prune(enable: boolean): this;
  targetNamespace(namespace: string): this;
  dependsOn(deps: Array<{ name: string; namespace?: string }>): this;
}

export function HelmRelease(name?: string): HelmReleaseBuilder;
export function Kustomization(name?: string): KustomizationBuilder;
