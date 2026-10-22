from pathlib import Path
import hashlib,json,os,shutil,subprocess
out=Path(__file__).resolve().parent
source=out/'runtime-source'
binding=json.loads((out/'source-candidate/source-binding.json').read_text())
docker=['docker','--host','unix:///Volumes/Asgard/Colima/cpar66-local-20261022/docker.sock']
env=os.environ.copy();env['DOCKER_API_VERSION']='1.53';env['HTTP_PROXY']='http://127.0.0.1:7897';env['HTTPS_PROXY']='http://127.0.0.1:7897';env['NO_PROXY']='localhost,127.0.0.1,::1'
artifact=out/'unsigned-artifact';artifact.mkdir(mode=0o755)
context=out/'release-image-context';context.mkdir()
version='0.1.0';target=binding['target'];revision=binding['base_commit']
options=['--revision',revision,'--rust-toolchain','1.97.1','--target',target,'--version',version]
tool=str(source/'scripts/p12-release-artifact.rb')
rows=[]
def run(command,name,expected=0):
 with (out/(name+'.log')).open('w') as f:r=subprocess.run(command,env=env,stdout=f,stderr=subprocess.STDOUT)
 row={'name':name,'command':command,'exit':r.returncode};rows.append(row)
 (out/'assembly-commands.json').write_text(json.dumps(rows,indent=2)+'\n')
 assert r.returncode==expected,row
 return row
binary=out/'build-output/gateway-aarch64-unknown-linux-gnu'
assert json.loads((out/'release-build.json').read_text())['exit']==0
shutil.copy2(binary,artifact/binary.name);os.chmod(artifact/binary.name,0o755)
run([tool,'build-metadata',*options,'--output',str(artifact/'gateway-build-metadata.json')],'build-metadata')
red=out/'red-two-file-artifact';red.mkdir();shutil.copy2(binary,red/binary.name);shutil.copy2(artifact/'gateway-build-metadata.json',red/'gateway-build-metadata.json')
run([tool,'manifest','--artifact-dir',str(red),*options,'--output',str(red/'artifact-manifest.json')],'red-two-file-artifact',1)
shutil.copy2(out/'build-output/gateway-sbom.cdx.json',artifact/'gateway-sbom.cdx.json')
run([tool,'identity','--repository','Ricardo121380/cpa-rust-gateway','--workflow-path','.github/workflows/release-artifact.yml','--ref','refs/heads/feature/62-client-metadata','--output',str(artifact/'signing-identity.json')],'expected-signing-identity')
shutil.copy2(binary,context/'gateway')
image='cpar66-candidate:'+revision[:8]
run(docker+['buildx','build','--builder','cpar66-release','--platform','linux/arm64','--file',str(source/'Dockerfile'),'--provenance=false','--build-arg','RELEASE_REVISION='+revision,'--build-arg','RELEASE_VERSION='+version,'--build-arg','RELEASE_RUST_VERSION=1.97.1','--build-arg','RELEASE_TARGET='+target,'--build-arg','RELEASE_BASE_IMAGE=debian:bookworm-slim@sha256:9b67294679b30e5d6ab257b40594feeb4a4b81f7fcf4131f4decf0d6a212a9b0','--build-arg','HTTP_PROXY=http://192.168.5.2:7897','--build-arg','HTTPS_PROXY=http://192.168.5.2:7897','--output',f'type=oci,dest={artifact}/gateway-image.oci.tar,name={image}','--output',f'type=docker,dest={out}/gateway-image.docker.tar,name={image}',str(context)],'oci-build')
run([tool,'manifest','--artifact-dir',str(artifact),*options,'--output',str(artifact/'artifact-manifest.json')],'manifest')
run([tool,'verify','--artifact-dir',str(artifact),*options],'strict-unsigned-verifier')
run(docker+['load','--input',str(out/'gateway-image.docker.tar')],'oci-docker-load')
run(docker+['run','--rm','--pull=never','--network','none','--read-only','--user','65532:65532',image,'--help'],'oci-runtime-help')
run(docker+['create','--name','cpar66-oci-byte-readback-737fd040','--network','none',image],'oci-byte-container')
run(docker+['cp','cpar66-oci-byte-readback-737fd040:/usr/local/bin/gateway',str(out/'oci-gateway-readback')],'oci-byte-readback')
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
assert sha(out/'oci-gateway-readback')==sha(binary)
(out/'oci-binary-binding.json').write_text(json.dumps({'status':'PASS','binary_sha256':sha(binary),'oci_embedded_binary_sha256':sha(out/'oci-gateway-readback'),'manifest_sha256':sha(artifact/'artifact-manifest.json'),'oci_sha256':sha(artifact/'gateway-image.oci.tar'),'docker_archive_sha256':sha(out/'gateway-image.docker.tar'),'signing':'NOT_RUN','cryptographic_verification':'NOT_RUN'},indent=2)+'\n')
(out/'oci-image-inspect.json').write_bytes(subprocess.check_output(docker+['image','inspect',image],env=env))
print(json.dumps({'status':'PASS','artifact':str(artifact),'binary_sha256':sha(binary),'manifest_sha256':sha(artifact/'artifact-manifest.json')}))
