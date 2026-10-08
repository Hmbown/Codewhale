import {create as linux} from '../../src/backends/linux.mjs';
import {create as harmonyos} from '../../src/backends/harmonyos.mjs';

export const calls=[];
export function create() {
  const refused=(...args)=>{calls.push(args);throw new Error('Fixture refused an unexpected desktop runner');};
  const exec={run:refused,runOk:refused,have:refused,shell:refused,readFile:refused,pullFile:refused};
  const factory={linux,harmonyos}[process.env.CU_SELECTOR_PLATFORM];
  if(!factory)throw new Error('Unknown fixture backend');
  return factory({exec});
}
