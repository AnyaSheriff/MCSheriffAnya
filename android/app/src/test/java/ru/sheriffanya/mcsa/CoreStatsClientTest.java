package ru.sheriffanya.mcsa;
import org.junit.Test;
import static org.junit.Assert.*;
public class CoreStatsClientTest {
    @Test public void cpuUsesElapsedTimeAndAllAvailableCpus() {
        assertEquals(12.5,CoreStatsClient.cpuPercent(1000000,2000000,1000,3000,4),0.00001);
        assertEquals(0,CoreStatsClient.cpuPercent(1000000,1000000,1000,3000,4),0.00001);
        assertEquals(100,CoreStatsClient.cpuPercent(0,100000000,0,1000,4),0.00001);
    }
    @Test public void missingOrRestartedCountersAreNotInventedZeroUsage() {
        assertTrue(Double.isNaN(CoreStatsClient.cpuPercent(-1,100,0,1000,1)));
        assertTrue(Double.isNaN(CoreStatsClient.cpuPercent(200,100,0,1000,1)));
        assertTrue(Double.isNaN(CoreStatsClient.cpuPercent(100,200,1000,1000,1)));
        assertTrue(Double.isNaN(CoreStatsClient.cpuPercent(100,200,0,1000,0)));
    }
}
