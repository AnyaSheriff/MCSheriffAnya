package ru.sheriffanya.mcsa;

import java.security.KeyFactory;
import java.security.PublicKey;
import java.security.Signature;
import java.security.spec.X509EncodedKeySpec;

/** Только стандартная ECDSA P-256/SHA-256. Закрытого ключа в приложении нет. */
final class CoreSignature {
    static boolean verify(byte[] publicKey,byte[] payload,byte[] signature) {
        if(publicKey.length>512||payload.length==0||payload.length>32768||signature.length<8||signature.length>80) return false;
        try {
            PublicKey key=KeyFactory.getInstance("EC").generatePublic(new X509EncodedKeySpec(publicKey));
            if(!(key instanceof java.security.interfaces.ECPublicKey)||((java.security.interfaces.ECPublicKey)key).getParams().getCurve().getField().getFieldSize()!=256) return false;
            Signature check=Signature.getInstance("SHA256withECDSA");check.initVerify(key);check.update(payload);return check.verify(signature);
        } catch(java.security.GeneralSecurityException invalid) { return false; }
    }
}
