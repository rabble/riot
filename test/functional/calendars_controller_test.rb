require File.dirname(__FILE__) + '/../test_helper'


class CalendarsControllerTest < ActionController::TestCase

  describe_requests do
    setup = lambda do
      #stub(@controller).current_user { admin_user }
    end

    context "GET /calendars/1" do
      before(&setup)
      
      act! { get :show, {:id => Calendar.make.id} }
      
      it_renders :template, :show
      it_assigns :calendar
    end
  end
end
