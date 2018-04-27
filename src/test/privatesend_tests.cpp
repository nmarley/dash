// Copyright (c) 2014-2018 The Dash Core developers
// Distributed under the MIT/X11 software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

#include "privatesend.h"
#include "test/test_dash.h"

#include <boost/test/unit_test.hpp>

BOOST_FIXTURE_TEST_SUITE(privatesend_tests, BasicTestingSetup)

/* Test STUFF */
BOOST_AUTO_TEST_CASE(privatesend_test)
{
    int nCount;
    nCount = privatesend.getCount();

    // privatesend.getEntries()
    BOOST_CHECK_EQUAL(nCount, 0);

    privatesend.add("mn1", "127.0.0.1:9999", "cT6dc7T42UrnDWbU1MAVr7fc6nCinH76FtY3GZVQJTiCpXuf2t5v", "a2be5bc8caefcde822cecfff34dad384aef165d8ae163a062dea43323ca63ac9", "1");
    privatesend.add("mn2", "127.0.0.2:9999", "cT6dc7T42UrnDWbU1MAVr7fc6nCinH76FtY3GZVQJTiCpXuf2t5x", "b2be5bc8caefcde822cecfff34dad384aef165d8ae163a062dea43323ca63ac9", "0");
    // should break (in theory)
    privatesend.add("mn2", "127.0.0.3:9999", "xT6dc7T42UrnDWbU1MAVr7fc6nCinH76FtY3GZVQJTiCpXuf2t5x", "b2be5bc8caefcde822cecfff34dad384aef165d8ae163a062dea43323ca63ac9", "0");

    nCount = privatesend.getCount();
    BOOST_CHECK_EQUAL(nCount, 2);

    const auto& mne = privatesend.findByAlias("mn1");
    BOOST_CHECK_EQUAL(mne.getPrivKey(), "cT6dc7T42UrnDWbU1MAVr7fc6nCinH76FtY3GZVQJTiCpXuf2t5v");

    const auto& mne2 = privatesend.findByAlias("mn2");
    BOOST_CHECK_EQUAL(mne2.getPrivKey(), "xT6dc7T42UrnDWbU1MAVr7fc6nCinH76FtY3GZVQJTiCpXuf2t5x");

    // break something so that I know tests are running
    BOOST_CHECK_EQUAL(0, 1);
}

BOOST_AUTO_TEST_SUITE_END()
