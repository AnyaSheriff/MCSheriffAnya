package ru.sheriffanya.mcsa;
import org.junit.Test;
import java.nio.charset.StandardCharsets;
import java.security.*;
import java.security.spec.ECGenParameterSpec;
import static org.junit.Assert.*;

public class CoreSignatureTest {
    private static KeyPair key() throws Exception { KeyPairGenerator gen=KeyPairGenerator.getInstance("EC");gen.initialize(new ECGenParameterSpec("secp256r1"));return gen.generateKeyPair(); }
    private static byte[] sign(KeyPair key,byte[] data) throws Exception { Signature s=Signature.getInstance("SHA256withECDSA");s.initSign(key.getPrivate());s.update(data);return s.sign(); }
    @Test public void acceptsOnlyAnAuthenticManifestWithPinnedKey() throws Exception {
        KeyPair key=key();byte[] data="actual manifest bytes".getBytes(StandardCharsets.UTF_8),signature=sign(key,data);
        assertTrue(CoreSignature.verify(key.getPublic().getEncoded(),data,signature));
        assertFalse(CoreSignature.verify(key().getPublic().getEncoded(),data,signature));
        data[0]^=1;assertFalse(CoreSignature.verify(key.getPublic().getEncoded(),data,signature));
    }
    @Test public void malformedSignaturesKeysAndOversizedDataAreRejected() throws Exception {
        KeyPair key=key();byte[] data={1,2,3},signature=sign(key,data);
        assertFalse(CoreSignature.verify(key.getPublic().getEncoded(),data,new byte[64]));
        assertFalse(CoreSignature.verify(new byte[20],data,signature));
        assertFalse(CoreSignature.verify(key.getPublic().getEncoded(),new byte[32769],signature));
        assertFalse(CoreSignature.verify(key.getPublic().getEncoded(),new byte[0],signature));
    }
}
