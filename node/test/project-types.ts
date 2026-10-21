import { projectAt, type ProjectLocation } from 'radicle-sdk';

async function projectTypes() {
  const location: ProjectLocation | null = await projectAt('.', { remoteName: 'peer' });
  if (location) {
    const rid: string = location.rid;
    const remote: string = location.remoteName;
    const namespace: string | undefined = location.namespace;
    void [rid, remote, namespace];
    // @ts-expect-error snapshots are readonly
    location.remoteName = 'other';
  }
  // @ts-expect-error path must be a string
  await projectAt(42);
  // @ts-expect-error remote name must be a string
  await projectAt('.', { remoteName: 42 });
}
void projectTypes;
