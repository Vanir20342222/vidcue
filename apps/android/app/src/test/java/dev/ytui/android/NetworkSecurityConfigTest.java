package dev.ytui.android;

import static org.junit.Assert.*;

import java.io.File;
import java.util.HashSet;
import java.util.Set;
import javax.xml.parsers.DocumentBuilderFactory;
import org.junit.Test;
import org.w3c.dom.Document;
import org.w3c.dom.Element;
import org.w3c.dom.NodeList;

/** AR-22: radio may use HTTP, YouTube and Google hosts may not. Reads the shipped sources. */
public final class NetworkSecurityConfigTest {
  private static final String[] PROTECTED = {
    "youtube.com",
    "youtu.be",
    "googlevideo.com",
    "ytimg.com",
    "ggpht.com",
    "googleusercontent.com",
    "google.com",
    "googleapis.com",
    "gstatic.com",
  };

  private static Document parse(String path) throws Exception {
    DocumentBuilderFactory factory = DocumentBuilderFactory.newInstance();
    factory.setNamespaceAware(true);
    return factory.newDocumentBuilder().parse(new File(path));
  }

  @Test
  public void baseAllowsCleartextForRadioAndLoopbackWithSystemTrust() throws Exception {
    Document config = parse("src/main/res/xml/network_security_config.xml");
    NodeList bases = config.getElementsByTagName("base-config");
    assertEquals(1, bases.getLength());
    Element base = (Element) bases.item(0);
    assertEquals("true", base.getAttribute("cleartextTrafficPermitted"));
    NodeList certificates = base.getElementsByTagName("certificates");
    assertEquals(1, certificates.getLength());
    assertEquals("system", ((Element) certificates.item(0)).getAttribute("src"));
  }

  @Test
  public void youtubeAndGoogleHostsNeverUseCleartext() throws Exception {
    Document config = parse("src/main/res/xml/network_security_config.xml");
    Set<String> forbidden = new HashSet<>();
    NodeList domains = config.getElementsByTagName("domain");
    for (int i = 0; i < domains.getLength(); i++) {
      Element domain = (Element) domains.item(i);
      Element owner = (Element) domain.getParentNode();
      if ("false".equals(owner.getAttribute("cleartextTrafficPermitted"))
          && "true".equals(domain.getAttribute("includeSubdomains")))
        forbidden.add(domain.getTextContent().trim());
    }
    for (String host : PROTECTED)
      assertTrue(host + " must forbid cleartext with subdomains", forbidden.contains(host));
  }

  @Test
  public void manifestUsesTheScopedConfigInsteadOfAppWideCleartext() throws Exception {
    Element application =
        (Element) parse("src/main/AndroidManifest.xml").getElementsByTagName("application").item(0);
    String android = "http://schemas.android.com/apk/res/android";
    assertEquals(
        "@xml/network_security_config",
        application.getAttributeNS(android, "networkSecurityConfig"));
    assertFalse(application.hasAttributeNS(android, "usesCleartextTraffic"));
  }
}
